//! 只有 root entry 的系统：被拒与超时也有记录、撤销、代际号、读取不在别处留下记录。
//! 参考：docs/Kernel/Journal.md（四条不变量）；docs/Kernel/Reference.md（撤销）。

use std::collections::BTreeMap;
use std::path::PathBuf;

use kernel::host::Config;
use kernel::index::{kernel_spec, what};
use kernel::kernel::{Conn, Kernel, KernelError};
use serde_json::{Value as Json, json};

struct Dir(PathBuf);
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn open(tag: &str) -> (Dir, Kernel) {
    let dir = Dir(std::env::temp_dir().join(format!("croon-kernel-{tag}-{}", std::process::id())));
    let _ = std::fs::remove_dir_all(&dir.0);
    let config = Config {
        languages: Vec::new(),
        budgets: croon_port::Budgets {
            nodes: 10000,
            read_bytes: 10_000_000,
            write_bytes: 10_000_000,
        },
        ports: BTreeMap::new(),
        clock: Box::new(|| "2026-01-01T00:00:00Z".to_owned()),
        fault: None,
    };
    let kernel = Kernel::open(&dir.0, config).unwrap();
    (dir, kernel)
}

fn any_port() -> Json {
    json!({"description": "收任何东西", "in": {}, "attach": {"additionalProperties": true}})
}

/// root entry：收件箱 inbox，me 接到它自己的 inbox，look 接到它自己的 Look。
fn root(kernel: &mut Kernel) -> (String, Conn) {
    let def = json!({
        "evaluator": "external",
        "interface": {
            "description": "测试用的 root entry",
            "signature": {"in": {}, "out": {}},
            "provides": {"inbox": any_port()},
            "requires": {"me": any_port(), "look": kernel_spec("look").0}
        }
    });
    let content = kernel.import(&def).unwrap();
    let bundle = kernel.import_bundle(content.as_str(), &[]).unwrap();
    let r = kernel
        .create_root(
            &bundle,
            &json!({"value":null,"dependencies":[]}),
            &json!({"me":"port/inbox","look":"look"}),
            &json!({"sources":{}}),
        )
        .unwrap();
    let conn = kernel.connect(&r).unwrap();
    (r, conn)
}

fn send(key: &str, to: Json, value: Json, attach: Json) -> Json {
    json!({"key": key, "send": {"to": to, "delivery":{"value":value,"dependencies":[]}, "attach": attach}})
}

fn last(kernel: &Kernel, instance: &str) -> Json {
    kernel
        .index()
        .journal(instance)
        .unwrap()
        .records
        .last()
        .unwrap()
        .clone()
}

fn len(kernel: &Kernel, instance: &str) -> usize {
    kernel.index().journal(instance).unwrap().records.len()
}

/// 求值过的键都有记录，包括被拒与取的超时。
#[test]
fn a_refused_send_and_a_timed_out_receive_are_recorded() {
    let (_d, mut k) = open("refused");
    let (r, c) = root(&mut k);
    let out = k
        .submit(
            &c,
            &send("0", json!({"handle": "h-nobody"}), json!(1), json!({})),
        )
        .unwrap()
        .unwrap();
    assert_eq!(out["outcome"]["error"]["kind"], "denied");
    let rec = last(&k, &r);
    let (kind, body) = what(&rec);
    assert_eq!((kind, &body["ref"]), ("send", &Json::Null));

    let out = k
        .submit(
            &c,
            &json!({"key": "1", "receive": {"ports": ["inbox"], "timeout": 0}}),
        )
        .unwrap()
        .unwrap();
    assert_eq!(out["arrival"], Json::Null);
    assert_eq!(what(&last(&k, &r)).1["result"], Json::Null);

    // 同一个键重报，得到同样的结果，不再记。
    let n = len(&k, &r);
    let again = k
        .submit(
            &c,
            &send("0", json!({"handle": "h-nobody"}), json!(1), json!({})),
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        again,
        k.submit(
            &c,
            &send("0", json!({"handle": "h-nobody"}), json!(1), json!({}))
        )
        .unwrap()
        .unwrap()
    );
    assert_eq!(len(&k, &r), n);
    // 同一个键报出不同的 Atom 是分歧：这次会话停下，要重新连上。
    assert!(matches!(
        k.submit(
            &c,
            &send("0", json!({"handle": "h-nobody"}), json!(2), json!({}))
        ),
        Err(KernelError::Refused(_))
    ));
    assert!(matches!(
        k.submit(&c, &send("2", json!({"port": "me"}), json!(1), json!({}))),
        Err(KernelError::Stale(_))
    ));
}

#[test]
fn an_old_session_cannot_write_and_leaves_nothing_behind() {
    let (_d, mut k) = open("fence");
    let (r, old) = root(&mut k);
    let new = k.connect(&r).unwrap();
    let n = len(&k, &r);
    let refused = k.submit(&old, &send("0", json!({"port": "me"}), json!(1), json!({})));
    assert!(matches!(refused, Err(KernelError::Stale(_))), "{refused:?}");
    assert_eq!(len(&k, &r), n);
    assert!(
        k.submit(&new, &send("0", json!({"port": "me"}), json!(1), json!({})))
            .unwrap()
            .is_some()
    );
}

#[test]
fn a_system_has_one_root() {
    let (_d, mut k) = open("oneroot");
    let (_, _) = root(&mut k);
    let def = json!({"evaluator": "external", "interface": {"description": "又一个", "signature": {"in": {}, "out": {}}, "provides": {}, "requires": {}}});
    let content = k.import(&def).unwrap();
    let bundle = k.import_bundle(content.as_str(), &[]).unwrap();
    assert!(matches!(
        k.create_root(
            &bundle,
            &json!({"value":null,"dependencies":[]}),
            &json!({}),
            &json!({"sources":{}})
        ),
        Err(KernelError::Refused(_))
    ));
}

/// 撤销沿派生链递归；持有者记一条失效，之后再用被拒；自己的一步在自己这本里造成的记录 by 是 self。
#[test]
fn revoking_invalidates_the_holder_and_later_use_is_denied() {
    let (_d, mut k) = open("revoke");
    let (r, c) = root(&mut k);
    let gave = k
        .submit(
            &c,
            &send(
                "give",
                json!({"port": "me"}),
                json!("给你回话的途径"),
                json!({"reply": {"port": "me"}}),
            ),
        )
        .unwrap()
        .unwrap();
    let derived = gave["derived"]["reply"]["handle"]
        .as_str()
        .unwrap()
        .to_owned();
    let revoker = gave["derived"]["reply"]["revoker"]["handle"]
        .as_str()
        .unwrap()
        .to_owned();
    let arrival = last(&k, &r);
    assert_eq!(arrival["by"], "self");
    assert_eq!(
        what(&arrival).1["attach"]["reply"]["handle"],
        derived.as_str()
    );

    let took = k
        .submit(&c, &json!({"key": "take", "receive": {"ports": ["inbox"]}}))
        .unwrap()
        .unwrap();
    assert_eq!(
        took["arrival"]["attach"]["reply"]["handle"],
        derived.as_str()
    );
    let used = k
        .submit(
            &c,
            &send("use", json!({"handle": derived}), json!(1), json!({})),
        )
        .unwrap()
        .unwrap();
    assert_eq!(used["outcome"]["ok"], true);

    let n = len(&k, &r);
    let revoked = k
        .submit(
            &c,
            &send("revoke", json!({"handle": revoker}), Json::Null, json!({})),
        )
        .unwrap()
        .unwrap();
    assert_eq!(revoked["outcome"]["ok"], true);
    let step = &k.index().journal(&r).unwrap().records[n];
    let invalid = last(&k, &r);
    assert_eq!(what(&invalid), ("invalid", &json!([derived])));
    assert_eq!(invalid["by"], "self");
    assert_eq!(invalid["event"], step["event"]);

    let after = k
        .submit(
            &c,
            &send("again", json!({"handle": derived}), json!(1), json!({})),
        )
        .unwrap()
        .unwrap();
    assert_eq!(after["outcome"]["error"]["kind"], "denied");
    assert_eq!(what(&last(&k, &r)).1["ref"], Json::Null);
    // 再撤销一次：已经撤销了，什么也不造成。
    let twice = k
        .submit(
            &c,
            &send("revoke2", json!({"handle": revoker}), Json::Null, json!({})),
        )
        .unwrap()
        .unwrap();
    assert_eq!(twice["outcome"]["ok"], true);
    assert!(last(&k, &r).get("event").is_none());
}

/// 查看与读取不在目标那里留下记录：读到的只是信息，其中的 Handle 不在读的一方的 c-list 里。
#[test]
fn reading_a_journal_leaves_no_record_and_gives_no_handles() {
    let (_d, mut k) = open("read");
    let (r, c) = root(&mut k);
    k.submit(
        &c,
        &send(
            "give",
            json!({"port": "me"}),
            json!(0),
            json!({"reply": {"port": "me"}}),
        ),
    )
    .unwrap();
    let n = len(&k, &r);
    let before: BTreeMap<_, _> = k.index().journal(&r).unwrap().acquired.clone();
    let read = k
        .submit(
            &c,
            &send(
                "read",
                json!({"port": "look"}),
                json!({"op": "read"}),
                json!({}),
            ),
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        read["outcome"]["value"]["records"]
            .as_array()
            .unwrap()
            .len(),
        n
    );
    let inspect = k
        .submit(
            &c,
            &send(
                "inspect",
                json!({"port": "look"}),
                json!({"op": "inspect"}),
                json!({}),
            ),
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        inspect["outcome"]["value"],
        json!({"status": "running", "outcome": null})
    );
    assert_eq!(len(&k, &r), n + 2, "两次交出各记一条，没有别的记录");
    assert_eq!(
        k.index().journal(&r).unwrap().acquired,
        before,
        "读到的 Handle 不进 c-list"
    );
}
