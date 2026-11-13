//! 消费方列出规格文档，共用展开与生成；生成表属于各自的构建产物。
//! 参考：docs/Kernel/Engineering.md（接口）。

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// 从给定文档生成 OUT_DIR/blocks.rs；不读取清单之外的文档。
pub fn generate(root: &Path, paths: &[&str], out_dir: &Path) {
    let blocks =
        crate::sources(root, paths).unwrap_or_else(|error| panic!("取不出文档里的 block：{error}"));
    let blocks = crate::expand(&blocks).unwrap_or_else(|errors| {
        panic!(
            "展开不了文档里的 $block：\n{}",
            errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    let mut out = String::from("pub static BLOCKS: &[Block] = &[\n");
    for block in blocks.values() {
        let file = Path::new(&block.file)
            .strip_prefix(root)
            .map_or_else(|_| block.file.clone(), |path| path.display().to_string());
        writeln!(
            out,
            "    Block {{ name: {:?}, lang: {:?}, text: {:?}, file: {:?}, line: {} }},",
            block.name, block.lang, block.text, file, block.line
        )
        .unwrap();
    }
    out.push_str("];\n");
    let target = out_dir.join("blocks.rs");
    std::fs::write(&target, out)
        .unwrap_or_else(|error| panic!("写不了 {}：{error}", target.display()));
    for path in paths {
        println!("cargo:rerun-if-changed={}", root.join(path).display());
    }
}

/// 构建脚本所在 crate 的仓库根与 OUT_DIR。
///
/// 仓库根是向上第一个含 `tracked.toml` 的目录；不按层数算，所以 crate 放进新的目录层级、
/// 或者几个仓库并排放在一起，都不用改。
pub fn build_paths() -> (PathBuf, PathBuf) {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo 给的"));
    let root = find_root(&manifest).unwrap_or_else(|| {
        panic!(
            "从 {} 向上找不到 tracked.toml，认不出仓库根",
            manifest.display()
        )
    });
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("cargo 给的"));
    (root, out)
}

/// 从 `dir` 向上找第一个含 `tracked.toml` 的目录。
pub fn find_root(dir: &Path) -> Option<PathBuf> {
    let mut current = dir.canonicalize().ok()?;
    loop {
        if current.join("tracked.toml").is_file() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}
