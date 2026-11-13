//! Croon 的 Kernel。形状以文档为权威，代码只在这里做文档承诺的事。

pub mod blocks;
pub mod definition;
pub mod host;
pub mod ids;
pub mod index;
pub mod journal;
pub mod kernel;
pub mod schema;
/// 内容存储的形状与实现住 `croon-port`；这里重导出，便于在 Kernel 之上写装的代码仍按 `kernel::store` 取用。
pub use croon_port::store;

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
