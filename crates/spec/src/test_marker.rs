//! 检查文档正文里的 `[test:文件:用例]` 标记。
//!
//! 文件相对仓库根，用例是那个文件里一个测试函数的名字：文件要存在，`fn 用例(` 要在
//! 文件里出现。只认正文里的标记，行内代码与 fence block 里的是示例，不算。

use std::ops::Range;
use std::path::Path;

use pulldown_cmark::{Event, Parser, Tag};

const OPEN: &str = "[test:";

/// 正文里的一个标记。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestMarker {
    pub file: String,
    pub line: usize,
    /// 测试文件，相对仓库根。
    pub path: String,
    pub case: String,
}

#[derive(Debug, thiserror::Error)]
pub enum TestMarkerError {
    #[error("{file}:{line}: `{marker}` 不是 `[test:文件:用例]` 的形状")]
    Malformed {
        file: String,
        line: usize,
        marker: String,
    },
    #[error("{file}:{line}: 测试文件 `{path}` 不存在")]
    MissingFile {
        file: String,
        line: usize,
        path: String,
    },
    #[error("{file}:{line}: `{path}` 里找不到用例 `{case}`")]
    MissingCase {
        file: String,
        line: usize,
        path: String,
        case: String,
    },
    #[error("{file}: {source}")]
    Io {
        file: String,
        #[source]
        source: std::io::Error,
    },
}

/// 取出 `source` 正文里的标记；形状不对的作为错误一并返回。`file` 只用于报错与记录来源。
pub fn test_markers(source: &str, file: &str) -> (Vec<TestMarker>, Vec<TestMarkerError>) {
    let code: Vec<Range<usize>> = Parser::new(source)
        .into_offset_iter()
        .filter(|(event, _)| matches!(event, Event::Code(_) | Event::Start(Tag::CodeBlock(_))))
        .map(|(_, range)| range)
        .collect();
    let mut markers = Vec::new();
    let mut errors = Vec::new();
    for (start, _) in source.match_indices(OPEN) {
        if code.iter().any(|range| range.contains(&start)) {
            continue;
        }
        let line = source[..start].matches('\n').count() + 1;
        let rest = &source[start + OPEN.len()..];
        let rest = &rest[..rest.find('\n').unwrap_or(rest.len())];
        let parsed = rest.find(']').and_then(|end| {
            let (path, case) = rest[..end].split_once(':')?;
            let (path, case) = (path.trim(), case.trim());
            (!path.is_empty() && !case.is_empty()).then_some((path, case))
        });
        match parsed {
            Some((path, case)) => markers.push(TestMarker {
                file: file.to_owned(),
                line,
                path: path.to_owned(),
                case: case.to_owned(),
            }),
            None => errors.push(TestMarkerError::Malformed {
                file: file.to_owned(),
                line,
                marker: format!("{OPEN}{}", rest.split(']').next().unwrap_or(rest)),
            }),
        }
    }
    (markers, errors)
}

/// 检查检查与取用的对象里的标记。没有标记时通过。
pub fn check_test_markers(root: &Path) -> Result<(), Vec<TestMarkerError>> {
    let files = crate::documents(root).map_err(|source| {
        vec![TestMarkerError::Io {
            file: root.display().to_string(),
            source,
        }]
    })?;
    let mut errors = Vec::new();
    for path in files {
        let file = path.display().to_string();
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(source) => {
                errors.push(TestMarkerError::Io { file, source });
                continue;
            }
        };
        let (markers, malformed) = test_markers(&text, &file);
        errors.extend(malformed);
        for marker in markers {
            let Ok(test) = std::fs::read_to_string(root.join(&marker.path)) else {
                errors.push(TestMarkerError::MissingFile {
                    file: marker.file,
                    line: marker.line,
                    path: marker.path,
                });
                continue;
            };
            if !defines(&test, &marker.case) {
                errors.push(TestMarkerError::MissingCase {
                    file: marker.file,
                    line: marker.line,
                    path: marker.path,
                    case: marker.case,
                });
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// `source` 里有没有 `fn case(`；`fn` 前面不能紧跟标识符字符，免得 `xfn` 也算。
fn defines(source: &str, case: &str) -> bool {
    let needle = format!("fn {case}(");
    source.match_indices(&needle).any(|(at, _)| {
        source[..at]
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_'))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers_in_code_are_examples() {
        let source = "\
正文 [test:crates/a.rs:one]，行内 `[test:x:y]` 不算。

```text
[test:x:y]
```

又一条 [test: crates/b.rs : two ]。
";
        let (markers, errors) = test_markers(source, "t.md");
        assert!(errors.is_empty());
        assert_eq!(markers.len(), 2);
        assert_eq!(
            (markers[0].path.as_str(), markers[0].case.as_str()),
            ("crates/a.rs", "one")
        );
        assert_eq!(markers[0].line, 1);
        assert_eq!(
            (markers[1].path.as_str(), markers[1].case.as_str()),
            ("crates/b.rs", "two")
        );
        assert_eq!(markers[1].line, 7);
    }

    #[test]
    fn a_marker_without_a_case_is_malformed() {
        let (markers, errors) = test_markers("一\n二 [test:crates/a.rs]\n", "t.md");
        assert!(markers.is_empty());
        assert!(
            matches!(&errors[..], [TestMarkerError::Malformed { line: 2, marker, .. }]
            if marker == "[test:crates/a.rs")
        );
    }

    #[test]
    fn a_case_must_be_a_whole_function_name() {
        assert!(defines("#[test]\nfn one() {}\n", "one"));
        assert!(!defines("fn one_more() {}\n", "one"));
        assert!(!defines("// afn one(\n", "one"));
    }

    #[test]
    fn missing_files_and_cases_are_reported() {
        let root = std::env::temp_dir().join(format!("spec-tests-{}", std::process::id()));
        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/a.rs"), "#[test]\nfn here() {}\n").unwrap();
        std::fs::write(
            root.join("docs/a.md"),
            "不变量 [test:src/a.rs:here]。\n不变量 [test:src/a.rs:gone]。\n不变量 [test:src/b.rs:here]。\n",
        )
        .unwrap();
        let errors = check_test_markers(&root).unwrap_err();
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(errors.len(), 2);
        assert!(
            matches!(&errors[0], TestMarkerError::MissingCase { line: 2, case, .. }
            if case == "gone")
        );
        assert!(
            matches!(&errors[1], TestMarkerError::MissingFile { line: 3, path, .. }
            if path == "src/b.rs")
        );
    }

    #[test]
    fn every_test_marker_resolves() {
        let root = crate::generate::find_root(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("find root");
        if let Err(errors) = check_test_markers(&root) {
            let report: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
            panic!("{} 个测试标记找不到：\n{}", report.len(), report.join("\n"));
        }
    }
}
