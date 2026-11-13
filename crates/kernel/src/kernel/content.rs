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
                crate::value::check_bytes(&bytes)?;
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
        if let Err(v) = crate::definition::check_shape(&d) {
            return Ok(Outcome::fail("use", format!("{v:?}")));
        }
        if d.get("evaluator").is_some() {
            return Ok(Outcome::fail(
                "use",
                "external Definition is created by host",
            ));
        }
        if !self.view().constructed(id) {
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
        if value["seats"].as_object().is_none_or(|s| !s.is_empty()) {
            return Ok(Outcome::fail("use", "第一版不实例化 Charter"));
        }
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
        if def.get("evaluator").is_some() || def.get("seats").is_some() {
            return Ok(Outcome::fail("use", "only Cognition can be instantiated"));
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
}
