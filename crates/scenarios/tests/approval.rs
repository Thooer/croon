//! 一个 Entry 交出两类意见；独立的确定求值器执行审批规则，共用场景驱动核对 Journal。
//! 参考：docs/Kernel/Scenarios/Approval.md；docs/Kernel/Definition/Session.md（会话）。

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use kernel::host::{CheckError, HostError, Language, Read, Session};
use scenarios::drive::Setup;
use scenarios::scenario::{self, Scn};
use serde_json::{Value as Json, json};

struct Approval;
struct ApprovalSession {
    results: BTreeMap<String, Json>,
    reported: BTreeSet<String>,
}

impl Language for Approval {
    fn id(&self) -> &str {
        "approval@1"
    }

    fn check(&self, definition: &Json, _: &dyn Read) -> Result<(), CheckError> {
        if definition["body"] != "approval" {
            return Err("审批求值器只解释 approval".into());
        }
        Ok(())
    }

    fn open(
        &self,
        _: &str,
        _: &Json,
        results: &BTreeMap<String, Json>,
        _: &dyn Read,
    ) -> Result<Box<dyn Session>, HostError> {
        Ok(Box::new(ApprovalSession {
            results: results.clone(),
            reported: BTreeSet::new(),
        }))
    }
}

impl Session for ApprovalSession {
    fn resume(
        &mut self,
        results: &BTreeMap<String, Json>,
        _: &dyn Read,
    ) -> Result<Json, HostError> {
        self.results.extend(results.clone());
        for port in ["left", "right"] {
            if self.results.contains_key(port) {
                continue;
            }
            return Ok(if self.reported.insert(port.into()) {
                json!({"atoms":[{"key":port,"receive":{"ports":[port]}}]})
            } else {
                json!({"atoms":[]})
            });
        }
        let approved = ["left", "right"]
            .into_iter()
            .all(|port| self.results[port]["arrival"]["delivery"]["value"] == true);
        Ok(json!({"done":{"ok":true,"value":{"approved":approved},"dependencies":[]}}))
    }
}

fn fixture() -> (Rc<Scn>, Setup) {
    let (scenarios, mut values) = scenario::kernel();
    let scenario = scenarios
        .into_iter()
        .find(|scenario| scenario.name == "scenario.approval")
        .expect("审批场景存在");
    let definition = values.get_mut("sha256:approval").unwrap();
    definition["$bundle"]["language"] = json!("approval@1");
    let setup = Setup::new(&values, || vec![Box::new(Approval)]);
    (Rc::new(scenario), setup)
}

#[test]
fn approval_uses_an_independent_evaluator_and_recovers_while_paused() {
    let (scenario, setup) = fixture();
    let run = setup.run(&scenario, 0, None);
    assert_eq!(run.crashes, 1, "第二项意见到达后、解除暂停之前重启");
}

#[test]
fn approval_recovers_at_every_commit_with_the_same_decision() {
    let (scenario, setup) = fixture();
    let baseline = setup.run(&scenario, 0, None);
    let commits = baseline.faults.borrow().commits;
    for n in 1..=commits {
        for after in [false, true] {
            let run = setup.run(&scenario, 0, Some((n, after)));
            assert!(run.crashes >= 1);
        }
    }
}
