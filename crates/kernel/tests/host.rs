//! 最小的宿主：不装 Language，root entry 直接经一个宿主 Port 交出、拿到结果；
//! 关掉 Kernel，用同一个目录重开，不再创建 root entry，重新 bind_host，按原键重报，
//! 直接得到同样的结果，Port 不再被调用。义务见本 crate 的头部。

use croon_port::store::Budgets;
use croon_port::{Call, Channel, Execution, HostDone, HostError, Port};
use kernel::host::Config;
use kernel::kernel::{Conn, Kernel};
use serde_json::{Value as Json, json};
use std::cell::Cell;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 原样交回请求的宿主 Port，数自己被调用了几次。
struct Echo {
    calls: Rc<Cell<usize>>,
}

impl Port for Echo {
    fn id(&self) -> &str {
        "echo"
    }
    fn channel(&self) -> Channel {
        Channel::Discardable
    }
    fn start(&self, call: &Call) -> Result<Box<dyn Execution>, HostError> {
        self.calls.set(self.calls.get() + 1);
        Ok(Box::new(Ready(HostDone {
            outcome: json!({"ok": true, "value": call.value, "dependencies": []}),
        })))
    }
    fn retire(&self, _attempt: &str) -> Result<(), HostError> {
        Ok(())
    }
}

struct Ready(HostDone);

impl Execution for Ready {
    fn collect(&mut self) -> Result<Option<HostDone>, HostError> {
        Ok(Some(self.0.clone()))
    }
}

fn spec() -> Json {
    json!({"description": "原样交回", "in": {}, "attach": {"additionalProperties": false},
        "out": {"value": {}, "attach": {"additionalProperties": false}}})
}

fn open(dir: &PathBuf) -> Kernel {
    Kernel::open(
        dir,
        Config {
            languages: Vec::new(),
            budgets: Budgets {
                nodes: 10_000,
                read_bytes: 1 << 20,
                write_bytes: 1 << 20,
            },
            ports: BTreeMap::new(),
            clock: Box::new(|| "2026-01-01T00:00:00Z".to_owned()),
            fault: None,
        },
    )
    .unwrap()
}

/// 装配：没有 root entry 就创建；每次打开都重新 bind_host，再取得执行权。
fn assemble(kernel: &mut Kernel, calls: &Rc<Cell<usize>>) -> Conn {
    let root = match kernel.index().root().map(ToString::to_string) {
        Some(root) => root,
        None => {
            let definition = json!({"evaluator": "external", "interface": {
                "description": "最小宿主的 root entry",
                "signature": {"in": {}, "out": {}},
                "provides": {},
                "requires": {"echo": spec()}
            }});
            let content = kernel.import(&definition).unwrap().to_string();
            let bundle = kernel.import_bundle(&content, &[]).unwrap();
            kernel
                .create_root(
                    &bundle,
                    &json!({"value": null, "dependencies": []}),
                    &json!({}),
                    &json!({"sources": {"echo": {"host": "echo", "spec": spec()}}}),
                )
                .unwrap()
        }
    };
    let handle = kernel.index().journals[root.as_str()].birth()["wire"]["echo"]
        .as_str()
        .unwrap()
        .to_owned();
    kernel
        .bind_host(
            &handle,
            Rc::new(Echo {
                calls: calls.clone(),
            }),
        )
        .unwrap();
    kernel.connect(&root).unwrap()
}

/// 客户端的一步：在途就推进别的 Instance，再用原键重报。
fn submit(kernel: &mut Kernel, conn: &Conn, atom: &Json) -> Json {
    loop {
        if let Some(result) = kernel.submit(conn, atom).unwrap() {
            return result;
        }
        assert!(kernel.stopped().is_empty(), "有 Instance 被宿主停下");
        kernel.run().unwrap();
    }
}

#[test]
fn a_minimal_host_assembles_submits_and_recovers_by_resubmitting_the_same_key() {
    let temp = Temp(std::env::temp_dir().join(format!(
        "croon-minimal-host-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )));
    let calls = Rc::new(Cell::new(0));
    let atom = json!({"key": "ask", "send": {
        "to": {"port": "echo"},
        "delivery": {"value": {"x": 1}, "dependencies": []},
        "attach": {}
    }});

    let mut kernel = open(&temp.0);
    let conn = assemble(&mut kernel, &calls);
    let first = submit(&mut kernel, &conn, &atom);
    assert_eq!(first["outcome"]["value"], json!({"x": 1}), "{first}");
    assert_eq!(calls.get(), 1);
    drop(kernel);

    // 同一个目录重开就是恢复：root entry 还在，按原键重报直接得到记下的结果。
    let mut kernel = open(&temp.0);
    let conn = assemble(&mut kernel, &calls);
    let again = submit(&mut kernel, &conn, &atom);
    assert_eq!(again["outcome"], first["outcome"]);
    assert_eq!(calls.get(), 1, "已提交的不再调用 Port");
}
