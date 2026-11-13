//! Delegate 用真正的 Lang0 求值，预期独立地写在文档里。
//! 参考：docs/Kernel/Scenarios/README.md（脚手架、扰动）。
use kernel::host::Language;
use lang0::Lang0;
use scenarios::{
    drive::Setup,
    scenario::{self, Scn},
};
use std::rc::Rc;

fn fixture() -> (Rc<Scn>, Setup) {
    let all = scenario::all();
    let values = scenario::values(&all);
    let (name, value) = all
        .iter()
        .find(|(n, _)| *n == "lang0.scenario.delegate")
        .unwrap();
    let setup = Setup::new(&values, || vec![Box::new(Lang0) as Box<dyn Language>]);
    (Rc::new(Scn::of(name, value)), setup)
}

#[test]
fn delegate_runs_with_real_lang0() {
    let (scn, setup) = fixture();
    setup.run(&scn, 0, None);
}

#[test]
fn delegate_has_the_same_journals_in_compatible_orders() {
    let (scn, setup) = fixture();
    for seed in 1..=8 {
        setup.run(&scn, seed, None);
    }
}

#[test]
fn delegate_recovers_at_every_commit() {
    let (scn, setup) = fixture();
    let baseline = setup.run(&scn, 0, None);
    let commits = baseline.faults.borrow().commits;
    for n in 1..=commits {
        for after in [false, true] {
            let run = setup.run(&scn, 0, Some((n, after)));
            assert!(run.crashes > 0);
        }
    }
}

#[test]
fn delegate_recovers_with_every_provider_atom_in_flight() {
    use kernel::index::what;
    use serde_json::json;

    let (scn, setup) = fixture();
    let baseline = setup.run(&scn, 0, None);
    let mut keys = Vec::new();
    for (instance, journal) in &scn.journals {
        let definition = what(&journal[0]).1["definition"].as_str().unwrap();
        if setup.values[definition]["$bundle"]["language"] != "lang0@1" {
            continue;
        }
        for record in journal {
            let (kind, body) = what(record);
            if kind == "send" && baseline.to_provider(instance, body) {
                keys.push((instance.clone(), body["key"].as_str().unwrap().to_owned()));
            }
        }
    }
    assert!(!keys.is_empty());
    for (instance, key) in &keys {
        let mut perturbed = Scn {
            name: scn.name.clone(),
            journals: scn.journals.clone(),
            pending: scn.pending.clone(),
            order: scn.order.clone(),
            perturb: scn.perturb.clone(),
        };
        perturbed
            .perturb
            .push(json!({"crash":{"instance":instance,"key":key}}));
        let run = setup.run(&Rc::new(perturbed), 0, None);
        assert_eq!(
            run.crashes, 1,
            "{instance} 的 {key} 必须在结果提交前崩溃一次"
        );
    }
    println!("{} 个 Lang0 llm/exec Atom 逐个在途崩溃并恢复", keys.len());
}
