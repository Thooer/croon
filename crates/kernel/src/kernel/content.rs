//! Kernel content operations use the explicit input scope and complete Definition identity.
//! 参考：docs/Kernel/Port.md（Kernel 的 Port）；docs/Kernel/Definition/README.md（完整身份与构造）。

use super::*;

impl Kernel {
    pub(super) fn pack(&self, value: &Json, content: &Content) -> Result<Outcome, KernelError> {
        let source = &value["content"];
        let id = if let Some(v) = source.get("json") {
            content.put_json(v).map_err(host_error)?
        } else if let Some(v) = source["base64"].as_str() {
            let bytes = match decode_base64(v) {
                Some(b) => b,
                None => return Ok(Outcome::fail("use", "invalid base64")),
            };
            content.put_bytes(&bytes).map_err(host_error)?
        } else if let Some(id) = source["id"].as_str() {
            if content.bytes(id).map_err(host_error)?.is_none() {
                return Ok(Outcome::rejected("denied", "content outside delivery"));
            }
            id.to_owned()
        } else {
            let mut bytes = Vec::new();
            for part in strings(&source["parts"]) {
                let Some(part) = content.bytes(&part).map_err(host_error)? else {
                    return Ok(Outcome::rejected("denied", "part outside delivery"));
                };
                bytes.extend(part);
                croon_eval::value::check_bytes(&bytes)?;
            }
            content.put_bytes(&bytes).map_err(host_error)?
        };
        let deps = strings(&value["dependencies"]);
        for d in &deps {
            if content.bundle(d).map_err(host_error)?.is_none() {
                return Ok(Outcome::rejected("denied", "dependency outside delivery"));
            }
        }
        let bundle = content.pack(&id, &deps).map_err(host_error)?;
        let mut o = Outcome::ok(json!({"id":bundle}));
        o.outcome["dependencies"] = json!([bundle]);
        Ok(o)
    }

    pub(super) fn construct(
        &self,
        value: &Json,
        content: &Content,
    ) -> Result<Outcome, KernelError> {
        let id = value["definition"].as_str().unwrap_or_default();
        let Some(bundle) = content.bundle(id).map_err(host_error)? else {
            return Ok(Outcome::rejected("denied", "Definition outside delivery"));
        };
        let Some(d) = content.json(&bundle.content).map_err(host_error)? else {
            return Ok(Outcome::fail("use", "Definition is not JSON"));
        };
        if let Err(v) = croon_eval::definition::check_shape(&d) {
            return Ok(Outcome::fail("use", format!("{v:?}")));
        }
        if d.get("seats").is_some() {
            let kernel = |op: &str| Some(crate::index::kernel_spec(op).0.clone());
            if let Err(v) = croon_eval::definition::check_charter(&d, &kernel) {
                return Ok(Outcome::fail("use", format!("{v:?}")));
            }
        }
        if d.get("evaluator").is_some() {
            return Ok(Outcome::fail(
                "use",
                "external Definition is created by host",
            ));
        }
        // Charter 没有 body，它的检查全在构造这一层的结构上，不经 Language。
        let charter = d.get("seats").is_some();
        if !charter && !self.view().constructed(id) {
            let lang = d["language"].as_str().unwrap_or_default();
            let Some(l) = self.config.languages.iter().find(|l| l.id() == lang) else {
                return Ok(Outcome::fail(
                    "use",
                    format!("Language {lang} not registered"),
                ));
            };
            let mut scope = Scope::default();
            scope.bundles.insert(id.to_owned());
            let fixed = Rc::new(
                Content::new(self.store.clone(), scope, self.config.budgets).map_err(host_error)?,
            );
            let reader = Reader::new(self, "", Some(fixed));
            match l.check(&d, &reader) {
                Ok(()) => {}
                Err(CheckError::Rejected(e)) => return Ok(Outcome::fail("use", e)),
                Err(CheckError::Host(e)) => return Err(host_error(e)),
            }
        }
        self.put_specs(&d)?;
        Ok(Outcome::ok(json!({"id":id})))
    }

    pub(super) fn instantiate(
        &mut self,
        i: &str,
        key: &str,
        via: &str,
        value: &Json,
        refs: &BTreeMap<String, String>,
        content: &Content,
    ) -> Result<Outcome, KernelError> {
        let def_id = value["definition"].as_str().unwrap_or_default();
        if content.bundle(def_id).map_err(host_error)?.is_none() {
            return Ok(Outcome::rejected("denied", "Definition outside delivery"));
        }
        if !self.view().constructed(def_id) {
            return Ok(Outcome::fail(
                "use",
                "Definition has no successful Kernel construct fact",
            ));
        }
        let Some(def) = self.view().definition_bundle(def_id) else {
            return Ok(Outcome::rejected("denied", "missing Definition"));
        };
        if def.get("evaluator").is_some() {
            return Ok(Outcome::fail("use", "only Cognition or Charter can be instantiated"));
        }
        if def.get("seats").is_some() {
            return self.charter_instantiate(i, key, via, &def_id, &def, value, refs, content);
        }
        if !content.grants(&value["input"]["dependencies"]) {
            return Ok(Outcome::rejected(
                "denied",
                "input dependencies outside delivery",
            ));
        }
        let iface = &def["interface"];
        let input = &value["input"];
        if let Err(v) = validate(&iface["signature"]["in"], &input["value"]) {
            let mut rejected = Outcome::rejected("type", "输入不合 Signature 的 in");
            rejected.outcome["error"]["data"] =
                json!(v.iter().map(ToString::to_string).collect::<Vec<_>>());
            return Ok(rejected);
        }
        let requires: BTreeSet<String> = iface["requires"]
            .as_object()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        let self_map = value
            .get("self")
            .and_then(Json::as_object)
            .cloned()
            .unwrap_or_default();
        for (port, target) in &self_map {
            let target = target.as_str().unwrap_or_default();
            let mine = &iface["requires"][port];
            let ok = requires.contains(port)
                && match target {
                    "look" => same_spec(kernel_spec("look").0, "", mine, ""),
                    t => t
                        .strip_prefix("port/")
                        .and_then(|p| iface["provides"].get(p))
                        .is_some_and(|p| same_spec(p, "", mine, "")),
                };
            if !ok {
                return Ok(Outcome::rejected(
                    "type",
                    format!("self 里的 {port} → {target} 接不上"),
                ));
            }
        }
        let wanted: BTreeSet<String> = requires
            .iter()
            .filter(|p| !self_map.contains_key(*p))
            .cloned()
            .collect();
        let given: BTreeSet<String> = refs.keys().cloned().collect();
        if wanted != given {
            return Ok(Outcome::rejected(
                "type",
                format!(
                    "附带要恰好是 Required port 里不在 self 的：要 {wanted:?}，给的是 {given:?}"
                ),
            ));
        }
        let child = self.ids.derive(IdKind::Instance, &[i, key]);
        let mut derived = serde_json::Map::new();
        let mut wire = serde_json::Map::new();
        for (name, parent) in refs {
            if self.correct_of(parent, &child) {
                return Ok(Outcome::rejected(
                    "denied",
                    "不能让一个 Instance 持有指向自己的 Correct",
                ));
            }
            let extra = View::spec_node_pub(&def, "requires", name, &child);
            if !self.wire_ok(parent, via, name, extra, None) {
                return Ok(Outcome::rejected(
                    "type",
                    format!("{name} 接上的 Reference 与 Required port 的 Port spec 不同"),
                ));
            }
            let d = self.derive(i, key, name);
            wire.insert(name.clone(), d["handle"].clone());
            derived.insert(name.clone(), d);
        }
        for port in self_map.keys() {
            wire.insert(port.clone(), json!(self.handle(&[&child, "self", port])));
        }
        let mut attach = serde_json::Map::new();
        for (port, spec) in iface["provides"].as_object().into_iter().flatten() {
            let name = format!("port/{port}");
            attach.insert(
                name.clone(),
                json!({"handle": self.handle(&[i, key, "result", &name]), "spec": spec}),
            );
        }
        for name in ["look", "notify", "correct"] {
            attach.insert(name.into(), json!({"handle": self.handle(&[i, key, "result", name]), "spec": kernel_spec(name).0}));
        }
        self.put_specs(&def)?;
        let input_id = self.store.put_json(&input["value"])?;
        self.store.put_json(&json!({}))?;
        let birth = json!({"birth": {"definition": def_id, "input":{"value":input_id.as_str(),"dependencies":input["dependencies"]}, "wire": wire}});
        let mut o = Outcome::ok(Json::Null);
        o.attach = Json::Object(attach);
        o.derived = Json::Object(derived);
        o.creates.push((child.clone(), raw(i, birth)));
        o.effects.push(Effect::Spawned(child));
        Ok(o)
    }
    /// 实例化一份 Charter：Collective 与它的全部成员在同一次事件里诞生。选人来自 pin 或本次请求；
    /// 成员接上的 Reference 按 Charter 的接线给出，来源不另记在成员那里，见 docs/Kernel/Journal.md 的「记录」。
    #[allow(clippy::too_many_arguments)]
    fn charter_instantiate(
        &mut self,
        i: &str,
        key: &str,
        via: &str,
        def_id: &str,
        def: &Json,
        value: &Json,
        refs: &BTreeMap<String, String>,
        content: &Content,
    ) -> Result<Outcome, KernelError> {
        let iface = &def["interface"];
        let input = &value["input"];
        if let Err(v) = validate(&iface["signature"]["in"], &input["value"]) {
            let mut rejected = Outcome::rejected("type", "输入不合 Signature 的 in");
            rejected.outcome["error"]["data"] =
                json!(v.iter().map(ToString::to_string).collect::<Vec<_>>());
            return Ok(rejected);
        }
        let seats = def["seats"].as_object().cloned().unwrap_or_default();
        let asked = value["seats"].as_object().cloned().unwrap_or_default();
        for name in asked.keys() {
            if !seats.contains_key(name) {
                return Ok(Outcome::rejected(
                    "type",
                    format!("Charter 没有名为 {name} 的 Seat"),
                ));
            }
        }

        // 选人：pin 写死的用 pin，其余由实例化的一方给。占座的 Definition 要与 Seat 的 Interface 相同。
        let mut staffing = serde_json::Map::new();
        let mut chosen: BTreeMap<String, (String, Json)> = BTreeMap::new();
        for (name, seat) in &seats {
            let empty = json!({"value": null, "dependencies": []});
            let (definition, seat_input) = match seat.get("pin") {
                Some(pin) => (
                    pin["definition"].as_str().unwrap_or_default().to_owned(),
                    pin.get("input").cloned().unwrap_or(empty),
                ),
                None => {
                    let Some(a) = asked.get(name) else {
                        return Ok(Outcome::rejected(
                            "type",
                            format!("没有写死的 Seat {name} 要在 seats 里给出人选"),
                        ));
                    };
                    (
                        a["definition"].as_str().unwrap_or_default().to_owned(),
                        a.get("input").cloned().unwrap_or(empty),
                    )
                }
            };
            if content.bundle(&definition).map_err(host_error)?.is_none() {
                return Ok(Outcome::rejected(
                    "denied",
                    format!("Seat {name} 的 Definition 不在本次交付里"),
                ));
            }
            if !self.view().constructed(&definition) {
                return Ok(Outcome::fail(
                    "use",
                    format!("Seat {name} 的 Definition 没有构造事实"),
                ));
            }
            let Some(member) = self.view().definition_bundle(&definition) else {
                return Ok(Outcome::rejected("denied", "missing Seat Definition"));
            };
            let want = &seat["interface"];
            let have = &member["interface"];
            if !same_croon_spec(&have["signature"], &want["signature"]) {
                return Ok(Outcome::rejected(
                    "type",
                    format!("占 Seat {name} 的 Definition 与它的 Signature 不同"),
                ));
            }
            for side in ["provides", "requires"] {
                let want_ports = want[side].as_object().cloned().unwrap_or_default();
                let have_ports = have[side].as_object().cloned().unwrap_or_default();
                if want_ports.len() != have_ports.len() {
                    return Ok(Outcome::rejected(
                        "type",
                        format!("占 Seat {name} 的 Definition 与它的 Interface 不同：{side}"),
                    ));
                }
                for (port, spec) in &want_ports {
                    let same = have_ports
                        .get(port)
                        .is_some_and(|h| same_croon_spec(h, spec));
                    if !same {
                        return Ok(Outcome::rejected(
                            "type",
                            format!(
                                "占 Seat {name} 的 Definition 与它的 Interface 不同：{side} 的 {port}"
                            ),
                        ));
                    }
                }
            }
            self.put_specs(&member)?;
            let seat_value = self.store.put_json(&seat_input["value"])?;
            staffing.insert(
                name.clone(),
                json!({"definition": definition, "input": {
                    "value": seat_value.as_str(),
                    "dependencies": seat_input["dependencies"]
                }}),
            );
            chosen.insert(name.clone(), (definition, seat_input));
        }

        // Collective 自己的 Required port：由实例化的一方接上，派生照常记在这次交出里。
        let mut derived = serde_json::Map::new();
        let mut wire = serde_json::Map::new();
        for port in iface["requires"].as_object().into_iter().flatten().map(|(p, _)| p) {
            let Some(parent) = refs.get(port) else {
                return Ok(Outcome::rejected(
                    "type",
                    format!("Collective 的 Required port {port} 没接上"),
                ));
            };
            if self.correct_of(parent, &self.ids.derive(IdKind::Instance, &[i, key])) {
                return Ok(Outcome::rejected(
                    "denied",
                    "不能让一个 Instance 持有指向自己的 Correct",
                ));
            }
            let extra = View::spec_node_pub(def, "requires", port, i);
            if !self.wire_ok(parent, via, port, extra, None) {
                return Ok(Outcome::rejected(
                    "type",
                    format!("{port} 接上的 Reference 与 Collective 的 Port spec 不同"),
                ));
            }
            let d = self.derive(i, key, port);
            wire.insert(port.clone(), d["handle"].clone());
            derived.insert(port.clone(), d);
        }

        // 成员的身份按位置算：同一个键重放得到同样的身份。
        let collective = self.ids.derive(IdKind::Instance, &[i, key]);
        let members: BTreeMap<String, String> = chosen
            .keys()
            .map(|name| {
                (
                    name.clone(),
                    self.ids.derive(IdKind::Instance, &[i, key, "seat", name]),
                )
            })
            .collect();

        let mut o = Outcome::ok(Json::Null);
        for (name, (definition, seat_input)) in &chosen {
            let child = &members[name];
            let mut member_wire = serde_json::Map::new();
            for port in def["wire"][name].as_object().into_iter().flatten().map(|(p, _)| p) {
                member_wire.insert(
                    port.clone(),
                    json!(self.handle(&[child, "wire", port])),
                );
            }
            let input_id = self.store.put_json(&seat_input["value"])?;
            let birth = json!({"birth": {
                "definition": definition,
                "input": {"value": input_id.as_str(), "dependencies": seat_input["dependencies"]},
                "wire": member_wire,
                "collective": collective,
                "seat": name
            }});
            o.creates.push((child.clone(), raw(i, birth)));
            o.effects.push(Effect::Spawned(child.clone()));
        }

        // 导出的 Provided port：外面的一方从占着那个 Seat 的成员的那一份派生，链上有两节。
        let mut attach = serde_json::Map::new();
        for (port, spec) in iface["provides"].as_object().into_iter().flatten() {
            if def["export"]["provides"][port]["seat"].as_str().is_none() {
                return Ok(Outcome::rejected("type", format!("导出的 {port} 没有 Seat")));
            }
            let name = format!("port/{port}");
            attach.insert(
                name.clone(),
                json!({"handle": self.handle(&[&collective, "export", port]), "spec": spec}),
            );
        }
        for name in ["look", "notify", "correct"] {
            attach.insert(
                name.into(),
                json!({"handle": self.handle(&[&collective, "result", name]), "spec": kernel_spec(name).0}),
            );
        }

        let input_id = self.store.put_json(&input["value"])?;
        let birth = json!({"birth": {
            "definition": def_id,
            "input": {"value": input_id.as_str(), "dependencies": input["dependencies"]},
            "wire": wire,
            "staffing": staffing
        }});
        o.creates.push((collective.clone(), raw(i, birth)));
        o.attach = Json::Object(attach);
        o.derived = Json::Object(derived);
        Ok(o)
    }
}

/// 两份 Port spec 或 Schema 相同：去掉描述之后规范文本相同。
fn same_croon_spec(a: &Json, b: &Json) -> bool {
    croon_eval::schema::json_eq(&croon_eval::port::strip(a), &croon_eval::port::strip(b))
}
