//! Croon 的 Kernel。形状以文档为权威，代码只在这里做文档承诺的事。

pub mod blocks;
pub mod definition;
pub mod host;
pub mod ids;
#[cfg(feature = "runtime")]
pub mod index;
#[cfg(feature = "runtime")]
pub mod journal;
#[cfg(feature = "runtime")]
pub mod kernel;
pub mod schema;
pub mod store;
pub mod value;

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
