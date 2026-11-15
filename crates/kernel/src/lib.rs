//! Croon 的 Kernel：一台机器上一个系统的运行时。形状以文档为权威，代码只在这里做文档承诺的事。
//!
//! 契约的公开面是 `croon-eval`、`croon-port` 与 `croon-tree`：提供者与 Language 只依赖它们，
//! 这里不重导出，形状与契约只有一个出处。本 crate 的消费者是宿主：系统外的根装配 Kernel、
//! Language 与宿主 Port，创建 root entry，再作为它的客户端在系统内行动。装起一个宿主之前必须知道的：
//!
//! - 装配的顺序：`Kernel::open` 打开目录，Language、预算在 `host::Config` 里；内容经
//!   `Kernel::store`（一个 `Rc<ContentStore>`，宿主可在上面构造 `Content`）或 `import` 导入；
//!   `Kernel::create_root` 的 binding 恰好覆盖 self 以外的每个 Required port；之后对每个宿主来源
//!   `Kernel::bind_host`，Port 的 `id()` 必须等于 binding 里写的恢复身份。同一个目录再打开时
//!   root entry 已经存在（`index().root()`），不再创建，只重新 bind_host。
//! - 客户端：`Kernel::connect` 取得执行权，代际加一，旧会话的交出被拒。每一步用固定的键
//!   `Kernel::submit`；返回 `None` 是在途，调用 `Kernel::run` 推进别的 Instance，再用原键重报
//!   同一个 Atom。同键不同 Atom 是重放分歧。已提交的直接得到结果，不会重做，所以宿主崩溃后
//!   用同一个目录重开、按原键重报就是恢复。
//! - 宿主停止：宿主 Port 返回 `HostError`、Language 会话故障这类宿主故障，Kernel 只停下受影响的
//!   Instance，记在 `Kernel::stopped`，不伪造结束，Notify 不送达。它不会自己恢复：修好原因后由宿主
//!   显式 `Kernel::reopen`；还有没确认的世界效果时被拒。等着它的客户端要自己看停止标记，否则会一直等。
//! - `host::Config::fault` 只用于测试提交窗口，生产给 `None`。
//!
//! 义务以上面几条为准，docs 不复述。理由：根与 root entry 见 docs/Kernel/README.md「根」；
//! 根绑定见 docs/Kernel/Port.md「根绑定」；会话、重连与代际见 docs/Kernel/Definition/Cognition.md
//! 「求值器在系统外」；停下、重开与重做见 docs/Kernel/Instance.md「执行」与「宿主故障与失败」。
//! 最小的宿主见 tests/host.rs。

pub mod blocks;
pub mod definition;
pub mod host;
pub mod ids;
pub mod index;
pub mod journal;
pub mod kernel;
pub mod schema;

#[cfg(test)]
pub(crate) mod testutil {
    //! 测试用的临时目录：用完就删。

    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    pub struct TempDir(pub PathBuf);

    impl TempDir {
        pub fn new(tag: &str) -> TempDir {
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("时钟在 1970 年之后")
                .as_nanos();
            let dir = std::env::temp_dir().join(format!(
                "croon-test-{tag}-{}-{nanos}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&dir).expect("建得了临时目录");
            TempDir(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
