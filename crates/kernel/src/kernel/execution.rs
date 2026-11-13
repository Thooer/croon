//! Accepted host execution snapshots and physical attempts, before and after Journal commit.
//! 参考：docs/Kernel/Instance.md（执行）；docs/Kernel/Engineering.md（接口）。

use super::*;

impl Kernel {
    fn acceptance_snapshot(
        &self,
        i: &str,
        value: &Json,
        dependencies: &[String],
        a: &AcceptedSend,
    ) -> Json {
        let nodes: Vec<Json> = a
            .chain
            .iter()
            .map(|n| json!({"spec":n.spec,"ptr":n.ptr,"author":n.author}))
            .collect();
        let snapshot = json!({"atom":a.atom,"reference":a.reference,"refs":a.refs,"chain":nodes,
            "source":match &a.provider {Some(Provider::Host(s))=>s.as_str(),_=>""},"provider":a.port.as_ref().unwrap().id(),
            "generation":self.live[i].generation,"value":value,"dependencies":dependencies});
        snapshot
    }
    fn persist_acceptance(
        &self,
        i: &str,
        key: &str,
        value: &Json,
        dependencies: &[String],
        a: &AcceptedSend,
    ) -> Result<(), KernelError> {
        self.journal.accept(
            &a.attempt,
            i,
            key,
            &self.acceptance_snapshot(i, value, dependencies, a),
        )?;
        Ok(())
    }

    pub(super) fn recover_attempts(&mut self) -> Result<(), KernelError> {
        for (attempt, i, key, committed, snapshot) in self.journal.attempts()? {
            if self
                .live
                .get(&i)
                .and_then(|l| l.inflight.get(&key))
                .is_some()
            {
                continue;
            }
            let identity = snapshot["provider"].as_str().unwrap_or_default();
            let port = self
                .config
                .ports
                .values()
                .find(|p| p.id() == identity)
                .cloned();
            let Some(port) = port else {
                self.stop(&i, format!("original recovery provider {identity} missing"))?;
                continue;
            };
            if port.channel() != Channel::Isolated {
                self.stop(&i, "original recovery channel differs".into())?;
                continue;
            }
            if let Err(e) = port.retire(&attempt) {
                self.stop(&i, format!("retire {attempt}: {e}"))?;
                continue;
            }
            if committed || self.index.journals[&i].keys.contains_key(&key) {
                self.journal.retire_attempt(&attempt)?;
                continue;
            }
            let source = snapshot["source"].as_str().unwrap_or_default().to_owned();
            let refs = snapshot["refs"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.clone(), v.as_str().map(str::to_owned)))
                .collect();
            let chain = snapshot["chain"]
                .as_array()
                .unwrap()
                .iter()
                .map(|n| Node {
                    spec: n["spec"].as_str().unwrap().to_owned(),
                    ptr: n["ptr"].as_str().unwrap().to_owned(),
                    author: n["author"].as_str().map(str::to_owned),
                })
                .collect();
            let mut scope = Scope::default();
            scope
                .values
                .insert(ValueId::of_json(&snapshot["value"])?.to_string());
            scope.bundles.extend(strings(&snapshot["dependencies"]));
            let content = Rc::new(
                Content::new(self.store.clone(), scope, self.config.budgets).map_err(host_error)?,
            );
            let generation = snapshot["generation"].as_u64().unwrap();
            let atom = snapshot["atom"].clone();
            // Replace the retired physical identity durably before a later start can occur.
            let next = self.ids.derive(
                IdKind::Event,
                &[
                    &i,
                    &key,
                    "attempt",
                    &self.journal.next_attempt()?.to_string(),
                ],
            );
            self.journal
                .replace_attempt(&attempt, &next, &i, &key, &snapshot)?;
            let external = self
                .view()
                .definition(&i)
                .is_some_and(|d| d.get("evaluator").is_some());
            let live = self
                .live
                .entry(i.clone())
                .or_insert_with(|| Live::new(external));
            live.generation = generation;
            live.inflight.insert(
                key,
                Flight {
                    atom: atom.clone(),
                    generation,
                    accepted: Some(AcceptedSend {
                        reference: snapshot["reference"].as_str().map(str::to_owned),
                        refs,
                        provider: Some(Provider::Host(source)),
                        chain,
                        channel: Some(Channel::Isolated),
                        execution: None,
                        completed: None,
                        content: Some(content),
                        port: Some(port),
                        attempt: next,
                        consumed: true,
                        atom,
                        started: false,
                    }),
                },
            );
        }
        let pending = self.journal.attempts()?;
        for i in self.stopped.keys() {
            let unresolved = pending.iter().any(|(_, owner, _, _, _)| owner == i)
                || self
                    .live
                    .get(i)
                    .is_some_and(|l| l.inflight.values().any(Flight::world_effect));
            self.stopped_world_effect.insert(i.clone(), unresolved);
            self.journal.set_stopped_world_effect(i, unresolved)?;
        }
        Ok(())
    }

    /// 接受时检查授权并固定通道；收取只使用已接受的执行对象。
    pub(super) fn dispatch(
        &mut self,
        i: &str,
        key: &str,
        value: &Json,
        dependencies: &[String],
        refs: &BTreeMap<String, String>,
        accepted: &mut AcceptedSend,
    ) -> Result<Option<Outcome>, KernelError> {
        let Some(provider) = accepted.provider.clone() else {
            return Ok(Some(Outcome::rejected(
                "denied",
                "这个 Reference 所指的 Port 解析不到",
            )));
        };
        let chain = &accepted.chain;
        let r = accepted.reference.as_deref().unwrap();
        if !accepted.consumed {
            if !canonical_dependencies(&accepted.atom["send"]["delivery"]["dependencies"]) {
                return Ok(Some(Outcome::rejected(
                    "denied",
                    "dependencies must be sorted unique Bundle IDs",
                )));
            }
            if chain.is_empty() {
                return Ok(Some(Outcome::rejected("denied", "unresolved chain")));
            }
            let names = refs.keys().cloned().collect();
            if let Some((kind, message)) = check_in(&self.view(), i, chain, value, &names) {
                return Ok(Some(Outcome::rejected(&kind, message)));
            }
            let scope = self.view().scope(i, None).map_err(host_error)?;
            let access =
                Content::new(self.store.clone(), scope, self.config.budgets).map_err(host_error)?;
            for dep in dependencies {
                if access.bundle(dep).map_err(host_error)?.is_none() {
                    return Ok(Some(Outcome::rejected(
                        "denied",
                        "request dependency not acquired",
                    )));
                }
            }
            let mut input_scope = Scope::default();
            input_scope
                .values
                .insert(ValueId::of_json(value)?.to_string());
            input_scope.bundles.extend(dependencies.iter().cloned());
            accepted.content = Some(Rc::new(
                Content::new(self.store.clone(), input_scope, self.config.budgets)
                    .map_err(host_error)?,
            ));
        }
        let o = match provider {
            Provider::Host(source) => {
                if accepted.execution.is_none() {
                    if accepted.channel == Some(Channel::Isolated)
                        && accepted.port.is_some()
                        && !accepted.attempt.is_empty()
                        && accepted.started
                    {
                        accepted
                            .port
                            .as_ref()
                            .unwrap()
                            .retire(&accepted.attempt)
                            .map_err(host_error)?;
                        let old = accepted.attempt.clone();
                        let next = self.ids.derive(
                            IdKind::Event,
                            &[i, key, "attempt", &self.journal.next_attempt()?.to_string()],
                        );
                        let mut scope = Scope::default();
                        scope.values.insert(ValueId::of_json(value)?.to_string());
                        scope.bundles.extend(dependencies.iter().cloned());
                        let content = Rc::new(
                            Content::new(self.store.clone(), scope, self.config.budgets)
                                .map_err(host_error)?,
                        );
                        let snapshot = self.acceptance_snapshot(i, value, dependencies, accepted);
                        self.journal
                            .replace_attempt(&old, &next, i, key, &snapshot)?;
                        accepted.attempt = next;
                        accepted.content = Some(content);
                        accepted.started = false;
                    }
                    let port = match &accepted.port {
                        Some(port) => port.clone(),
                        None => self.config.ports.get(&source).cloned().ok_or_else(|| {
                            KernelError::Host(format!("missing host source {source}"))
                        })?,
                    };
                    let binding = self
                        .index
                        .birth_wire
                        .get(&source)
                        .ok_or_else(|| KernelError::Host("missing source identity".into()))?;
                    let identity = self.index.journals[&binding.0].birth()["binding"]["sources"]
                        [&binding.1]["host"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned();
                    if identity != port.id() {
                        return Err(KernelError::Host(
                            "original provider identity missing".into(),
                        ));
                    }
                    accepted.channel = Some(port.channel());
                    accepted.port = Some(port.clone());
                    accepted.consumed = true;
                    if accepted.attempt.is_empty() {
                        accepted.attempt = self.ids.derive(
                            IdKind::Event,
                            &[i, key, "attempt", &self.journal.next_attempt()?.to_string()],
                        );
                    }
                    if port.channel() == Channel::Isolated {
                        self.persist_acceptance(i, key, value, dependencies, accepted)?;
                    }
                    let call = Call {
                        instance: i,
                        generation: self.live[i].generation,
                        key,
                        source: &source,
                        attempt: &accepted.attempt,
                        value,
                        dependencies,
                        content: accepted.content.as_ref().unwrap().clone(),
                    };
                    accepted.started = true;
                    accepted.execution = Some(port.start(&call).map_err(host_error)?);
                }
                if accepted.completed.is_none() {
                    let Some(done) = accepted
                        .execution
                        .as_mut()
                        .unwrap()
                        .collect()
                        .map_err(host_error)?
                    else {
                        return Ok(None);
                    };
                    accepted.completed = Some(done);
                }
                let done = accepted.completed.as_ref().unwrap().clone();
                if validate(outcome_schema(), &done.outcome).is_err()
                    || !canonical_dependencies(&done.outcome["dependencies"])
                    || !accepted
                        .content
                        .as_ref()
                        .unwrap()
                        .grants(&done.outcome["dependencies"])
                {
                    return Err(KernelError::Host(
                        "host outcome or output dependencies violate Content scope".into(),
                    ));
                }
                Outcome::of(done.outcome)
            }
            Provider::Kernel(k) => match k.as_str() {
                "pack" => self.pack(value, accepted.content.as_ref().unwrap())?,
                "construct" => self.construct(value, accepted.content.as_ref().unwrap())?,
                "instantiate" => {
                    self.instantiate(i, key, r, value, refs, accepted.content.as_ref().unwrap())?
                }
                other => {
                    Outcome::rejected("denied", format!("Kernel 的 Port {other} 不能这样接上"))
                }
            },
            Provider::Look(t) => self.look(&t, value)?,
            Provider::Notify(t) => self.notify(i, key, r, &t, value, refs)?,
            Provider::Correct(t) => self.correct(i, key, r, &t, value, refs)?,
            Provider::Revoke(d) => self.revoke(i, &d),
            Provider::Provided { instance, port } => {
                self.deliver(i, key, r, &instance, &port, value, dependencies, refs)?
            }
        };
        accepted.consumed |= o.accepted;
        Ok(Some(self.checked(o, chain)))
    }
}
