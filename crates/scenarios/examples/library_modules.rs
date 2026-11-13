//! 打印 `library/` 里生成的三份模块，供 CPython 上的参考实现读。
//! 用法：cargo run -p scenarios --features lang0 --example library_modules -- <输出目录>
fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| panic!("用法：library_modules <输出目录>"));
    for (name, source) in scenarios::expand::generated_modules() {
        let path = std::path::Path::new(&out).join(name);
        std::fs::write(&path, source).unwrap_or_else(|e| panic!("写不了 {}：{e}", path.display()));
        println!("{}", path.display());
    }
}
