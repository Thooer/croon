//! `tracked.toml`：这个仓库里哪些文件算数。快照收哪些，上下文占用数哪些，各一份清单。

use std::collections::HashSet;
use std::path::Path;

/// 清单，路径都相对仓库根。顶层两个管快照，`context` 管上下文占用，互不相干。
pub struct Config {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub context: Context,
}

/// `[context]`：算上下文占用的名单，写法与顶层相同。
pub struct Context {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

pub fn config(root: &Path) -> Result<Config, String> {
    let path = root.join("tracked.toml");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let table: toml::Table = text
        .parse()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let file = path.display().to_string();

    let mut include = None;
    let mut exclude = None;
    let mut context = None;
    for (key, value) in &table {
        match key.as_str() {
            "include" => include = Some(strings(&file, key, value)?),
            "exclude" => exclude = Some(strings(&file, key, value)?),
            "context" => {
                let inner = value
                    .as_table()
                    .ok_or_else(|| format!("{file}: `context` 要是一张表"))?;
                let mut c_include = None;
                let mut c_exclude = None;
                for (key, value) in inner {
                    let name = format!("context.{key}");
                    match key.as_str() {
                        "include" => c_include = Some(strings(&file, &name, value)?),
                        "exclude" => c_exclude = Some(strings(&file, &name, value)?),
                        _ => return Err(format!("{file}: 没有 `{name}` 这一项")),
                    }
                }
                context = Some(Context {
                    include: c_include.ok_or_else(|| format!("{file}: 缺 `context.include`"))?,
                    exclude: c_exclude.unwrap_or_default(),
                });
            }
            other => return Err(format!("{file}: 没有 `{other}` 这一项")),
        }
    }

    Ok(Config {
        include: include.ok_or_else(|| format!("{file}: 缺 `include`"))?,
        exclude: exclude.unwrap_or_default(),
        context: context.ok_or_else(|| format!("{file}: 缺 `[context]`"))?,
    })
}

fn strings(file: &str, key: &str, value: &toml::Value) -> Result<Vec<String>, String> {
    let items = value
        .as_array()
        .ok_or_else(|| format!("{file}: `{key}` 要是字符串数组"))?;
    items
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("{file}: `{key}` 里只能写字符串：{item:?}"))
        })
        .collect()
}

/// 按 `include` 的顺序取出这些路径，去重；`exclude` 命中的去掉。`section` 是清单所在的
/// 表名加点，顶层的写空串，只用在报错里。两个清单里的每一项都要有对应的文件，写错了
/// 当场报出来。
pub fn select<'a>(
    section: &str,
    include: &[String],
    exclude: &[String],
    paths: &[&'a str],
) -> Result<Vec<&'a str>, String> {
    let excluded = |path: &str| exclude.iter().any(|entry| matched(path, entry));
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for entry in include {
        let mut hit = 0;
        for path in paths {
            if matched(path, entry) && !excluded(path) {
                hit += 1;
                if seen.insert(*path) {
                    out.push(*path);
                }
            }
        }
        if hit == 0 {
            return Err(format!(
                "tracked.toml: {section}include 里的 `{entry}` 没有对应的文件"
            ));
        }
    }
    for entry in exclude {
        if !paths.iter().any(|path| matched(path, entry)) {
            return Err(format!(
                "tracked.toml: {section}exclude 里的 `{entry}` 没有对应的文件"
            ));
        }
    }
    Ok(out)
}

/// 一项是文件夹（以 `/` 结尾）就按前缀匹配，否则按整个路径匹配。
pub fn matched(path: &str, entry: &str) -> bool {
    if entry.ends_with('/') {
        path.starts_with(entry)
    } else {
        path == entry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_follows_include_order_and_drops_excludes() {
        let include = vec!["b.md".to_owned(), "docs/".to_owned()];
        let exclude = vec!["docs/Draft.md".to_owned()];
        let paths = ["docs/Draft.md", "docs/a.md", "a.md", "b.md"];
        let selected = select("", &include, &exclude, &paths).unwrap();
        assert_eq!(selected, ["b.md", "docs/a.md"]);
    }

    #[test]
    fn a_path_that_matches_nothing_is_an_error() {
        let include = vec!["nosuch/".to_owned()];
        let error = select("context.", &include, &[], &["a.md"]).unwrap_err();
        assert!(
            error.contains("context.include") && error.contains("nosuch/"),
            "{error}"
        );
    }
}
