//! 脚手架拿场景驱动真的 Kernel：内容库、记录库、索引、一次事务的决定与提交、恢复都是真的。
//! 参考：docs/Kernel/Scenarios/README.md（脚手架、扰动）；docs/Kernel/Journal.md（记录、四条不变量、实现）。

mod script;

use std::collections::BTreeSet;
use std::rc::Rc;

use kernel::host::Language;
use kernel::index::{Index, Provider, View, what};
use scenarios::drive::Setup;
use scenarios::scenario::{self, Scn};
use serde_json::Value as Json;

fn kernel_scenarios() -> (Vec<Rc<Scn>>, Setup) {
    let (scns, values) = scenario::kernel();
    (
        scns.into_iter().map(Rc::new).collect(),
        Setup::fake(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."),
            &values,
        ),
    )
}

// ---- fake@1：Atom 由脚手架按预期报出 ----

#[test]
fn scenarios_run_on_the_real_kernel() {
    let (scns, setup) = kernel_scenarios();
    for scn in &scns {
        setup.run(scn, 0, None);
    }
}

#[test]
fn any_compatible_order_gives_the_same_journals() {
    let (scns, setup) = kernel_scenarios();
    for scn in &scns {
        for seed in 1..=8 {
            setup.run(scn, seed, None);
        }
    }
}

macro_rules! commit_crash_test {
    ($name:ident, $scenario:literal) => {
        #[test]
        fn $name() {
            let (scns, setup) = kernel_scenarios();
            let scn = scns.iter().find(|s| s.name == $scenario).unwrap();
            let commits = setup.run(scn, 0, None).faults.borrow().commits;
            assert!(commits > 3, "{}：只数到 {commits} 个事务", scn.name);
            for n in 1..=commits {
                for after in [false, true] {
                    let r = setup.run(scn, n as u64, Some((n, after)));
                    assert!(r.crashes >= 1, "{}：第 {n} 个事务处应当退出过", scn.name);
                }
            }
        }
    };
}

commit_crash_test!(crash_quick_at_any_commit, "scenario.quick");
commit_crash_test!(crash_delegate_at_any_commit, "scenario.delegate");
commit_crash_test!(crash_forge_at_any_commit, "scenario.forge");
commit_crash_test!(crash_race_at_any_commit, "scenario.race");
commit_crash_test!(crash_pauseonly_at_any_commit, "scenario.pauseonly");
commit_crash_test!(crash_reconnect_at_any_commit, "scenario.reconnect");
commit_crash_test!(crash_approval_at_any_commit, "scenario.approval");
commit_crash_test!(crash_binding_at_any_commit, "scenario.binding");

#[test]
fn the_index_is_the_same_in_any_rebuild_order() {
    let (scns, setup) = kernel_scenarios();
    for scn in &scns {
        let r = setup.run(scn, 0, None);
        let kernel = r.kernel.as_ref().unwrap();
        let all = kernel.journal().all().unwrap();
        let mut rng = 0x2545_F491_4F6C_DD1Du64;
        for _ in 0..16 {
            let mut cursors: Vec<(String, std::collections::VecDeque<Json>)> = all
                .iter()
                .map(|(i, rs)| (i.clone(), rs.iter().cloned().collect()))
                .collect();
            let mut ix = Index::default();
            while cursors.iter().any(|(_, q)| !q.is_empty()) {
                rng ^= rng << 13;
                rng ^= rng >> 7;
                rng ^= rng << 17;
                let live: Vec<usize> = (0..cursors.len())
                    .filter(|&n| !cursors[n].1.is_empty())
                    .collect();
                let n = live[(rng % live.len() as u64) as usize];
                let record = cursors[n].1.pop_front().unwrap();
                ix.apply(&cursors[n].0.clone(), record);
            }
            assert!(
                ix == *kernel.index(),
                "{}：换一个次序重建，索引不同",
                scn.name
            );
        }
    }
}

/// 一个 Instance 影响另一个，当且仅当它的某一步在对方的 Journal 里造成了一条记录：
/// 取、读取、交给 engine 或 exec 都不在别处留下记录。
#[test]
fn only_sends_and_corrections_leave_records_elsewhere() {
    let (scns, setup) = kernel_scenarios();
    for scn in &scns {
        let r = setup.run(scn, 0, None);
        let kernel = r.kernel.as_ref().unwrap();
        let ix = kernel.index();
        let view = View {
            ix,
            store: kernel.store(),
        };
        let mut elsewhere: BTreeSet<String> = BTreeSet::new();
        for j in ix.journals.values() {
            for rec in &j.records {
                if rec["by"] != "self"
                    && let Some(e) = rec["event"].as_str()
                {
                    elsewhere.insert(e.to_owned());
                }
            }
        }
        for (i, j) in &ix.journals {
            for rec in &j.records {
                let (kind, body) = what(rec);
                let Some(e) = rec["event"].as_str() else {
                    continue;
                };
                if rec["by"] != "self" || !elsewhere.contains(e) {
                    continue;
                }
                match kind {
                    "end" => {}
                    "send" => {
                        let p = view.provider(body["ref"].as_str().unwrap());
                        assert!(
                            matches!(
                                p,
                                Some(
                                    Provider::Provided { .. }
                                        | Provider::Correct(_)
                                        | Provider::Revoke(_)
                                )
                            ) || p == Some(Provider::Kernel("instantiate".into())),
                            "{}：{i} 经 {p:?} 的交出在别处留下了记录",
                            scn.name
                        );
                    }
                    other => panic!("{}：{i} 的 {other} 在别处留下了记录", scn.name),
                }
            }
        }
    }
}

// ---- 真的 Language：Atom 由它报出，脚手架只决定 engine 与 exec 何时给出结果 ----

fn script() -> Vec<Box<dyn Language>> {
    vec![Box::new(script::Script)]
}

/// Kernel 的场景，fake@1 的 Definition 换成 script@1。Quick 不在其中：S 先结束靠脚手架控制报出，
/// 真的 Language 里 W0 的登记紧接着交办，由 Kernel 当场完成，脚手架管不着这个先后。
fn scripted_scenarios() -> Vec<(Rc<Scn>, Setup)> {
    let (scns, values) = scenario::kernel();
    scns.into_iter()
        .filter(|s| !matches!(s.name.as_str(), "scenario.quick" | "scenario.binding"))
        .map(|s| {
            let values = script::scripted(&s, &values);
            (
                Rc::new(s),
                Setup::new(
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."),
                    &values,
                    script,
                ),
            )
        })
        .collect()
}

#[test]
fn a_real_language_reports_its_own_atoms_in_any_compatible_order() {
    let all = scripted_scenarios();
    assert!(all.len() >= 5);
    for (scn, setup) in &all {
        let real = setup.run(scn, 0, None).shared.borrow().real.clone();
        assert!(
            !real.is_empty(),
            "{}：没有真的 Language 的 Instance",
            scn.name
        );
        for seed in 1..=8 {
            setup.run(scn, seed, None);
        }
    }
}

#[test]
fn a_real_language_recovers_from_a_crash_at_any_commit() {
    // Delegate 覆盖会话自行报出 Atom、等待与结果派生；Kernel 各机制的恢复由 fake@1 场景承担。
    let (scn, setup) = scripted_scenarios()
        .into_iter()
        .find(|(scn, _)| scn.name == "scenario.delegate")
        .expect("Delegate 场景存在");
    let commits = setup.run(&scn, 0, None).faults.borrow().commits;
    for n in 1..=commits {
        for after in [false, true] {
            let r = setup.run(&scn, n as u64, Some((n, after)));
            assert!(r.crashes >= 1, "{}：第 {n} 个事务处应当退出过", scn.name);
        }
    }
}
