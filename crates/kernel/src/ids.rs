//! Instance 的身份、Handle、事件 ID。
//! 参考：docs/Kernel/Instance.md（ID）。
//!
//! 它们不是 Value，不按内容寻址。ID 由造成它的位置确定地算出：同样的输入得到同样的 Journal。

use std::sync::OnceLock;

use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IdKind {
    Instance,
    Handle,
    Event,
}

/// 文档里 `instance.id` 的字段。
struct Format {
    prefixes: [(IdKind, String); 3],
    separator: String,
    alphabet: String,
    max_length: usize,
}

fn format() -> &'static Format {
    static FORMAT: OnceLock<Format> = OnceLock::new();
    FORMAT.get_or_init(|| {
        let block = crate::blocks::json("instance.id");
        let text = |value: &serde_json::Value, what: &str| {
            value
                .as_str()
                .unwrap_or_else(|| panic!("instance.id 缺少字符串字段 {what}"))
                .to_owned()
        };
        let prefix = |name: &str| text(&block["prefix"][name], &format!("prefix.{name}"));
        Format {
            prefixes: [
                (IdKind::Instance, prefix("instance")),
                (IdKind::Handle, prefix("handle")),
                (IdKind::Event, prefix("event")),
            ],
            separator: text(&block["separator"], "separator"),
            alphabet: text(&block["alphabet"], "alphabet"),
            max_length: block["max_length"]
                .as_u64()
                .expect("instance.id 缺少整数字段 max_length") as usize,
        }
    })
}

fn prefix_of(kind: IdKind) -> &'static str {
    &format()
        .prefixes
        .iter()
        .find(|(k, _)| *k == kind)
        .expect("三种 ID 都有前缀")
        .1
}

/// 认出一个字符串是哪种 ID；格式不对的返回 None。
/// Value 的 ID（`sha256:` 开头）与 `self`、`outside` 都不是这三种。
pub fn kind_of(id: &str) -> Option<IdKind> {
    let f = format();
    if id.len() > f.max_length {
        return None;
    }
    let (kind, body) = f.prefixes.iter().find_map(|(kind, prefix)| {
        let rest = id.strip_prefix(prefix.as_str())?;
        Some((*kind, rest.strip_prefix(f.separator.as_str())?))
    })?;
    let ok = !body.is_empty() && body.chars().all(|c| f.alphabet.contains(c));
    ok.then_some(kind)
}

/// 一个系统的 ID 生成器。密钥属于这个系统：没有它，算不出也猜不中别人的 ID。
#[derive(Clone)]
pub struct IdGen {
    key: Vec<u8>,
}

impl IdGen {
    pub fn new(key: &[u8]) -> IdGen {
        IdGen { key: key.to_vec() }
    }

    /// 由密钥、种类与造成它的位置算出。位置的每一部分都带长度，所以
    /// `["ab", "c"]` 与 `["a", "bc"]` 是两个位置。
    pub fn derive(&self, kind: IdKind, position: &[&str]) -> String {
        let mut hasher = Sha256::new();
        let part = |hasher: &mut Sha256, bytes: &[u8]| {
            hasher.update((bytes.len() as u64).to_be_bytes());
            hasher.update(bytes);
        };
        part(&mut hasher, &self.key);
        part(&mut hasher, prefix_of(kind).as_bytes());
        for p in position {
            part(&mut hasher, p.as_bytes());
        }
        let mut id = String::new();
        id.push_str(prefix_of(kind));
        id.push_str(&format().separator);
        for byte in hasher.finalize() {
            use std::fmt::Write as _;
            write!(id, "{byte:02x}").expect("写进 String 不会失败");
        }
        assert!(
            id.len() <= format().max_length,
            "算出的 ID 超过 instance.id 的 max_length"
        );
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::ValueId;

    fn gen_() -> IdGen {
        IdGen::new(b"test-system")
    }

    #[test]
    fn the_same_position_always_gives_the_same_id() {
        assert_eq!(
            gen_().derive(IdKind::Handle, &["i-root", "7", "0"]),
            gen_().derive(IdKind::Handle, &["i-root", "7", "0"])
        );
    }

    #[test]
    fn kind_position_and_key_each_change_the_id() {
        let base = gen_().derive(IdKind::Handle, &["a", "1"]);
        assert_ne!(base, gen_().derive(IdKind::Event, &["a", "1"]));
        assert_ne!(base, gen_().derive(IdKind::Handle, &["a", "2"]));
        assert_ne!(
            base,
            IdGen::new(b"other-system").derive(IdKind::Handle, &["a", "1"])
        );
    }

    #[test]
    fn position_parts_cannot_be_shifted_into_each_other() {
        let g = gen_();
        assert_ne!(
            g.derive(IdKind::Instance, &["ab", "c"]),
            g.derive(IdKind::Instance, &["a", "bc"])
        );
        assert_ne!(
            g.derive(IdKind::Instance, &["a"]),
            g.derive(IdKind::Instance, &["a", ""])
        );
    }

    #[test]
    fn derived_ids_are_recognised_by_kind_and_fit_the_limit() {
        for kind in [IdKind::Instance, IdKind::Handle, IdKind::Event] {
            let id = gen_().derive(kind, &["x"]);
            assert_eq!(kind_of(&id), Some(kind), "{id}");
        }
    }

    #[test]
    fn the_three_kinds_never_look_like_each_other_or_like_a_value_id() {
        let g = gen_();
        let ids: Vec<String> = [IdKind::Instance, IdKind::Handle, IdKind::Event]
            .iter()
            .map(|k| g.derive(*k, &["x"]))
            .collect();
        for (i, a) in ids.iter().enumerate() {
            for (j, b) in ids.iter().enumerate() {
                assert_eq!(i == j, kind_of(a) == kind_of(b));
            }
            assert!(ValueId::parse(a).is_none());
        }
        assert_eq!(kind_of(ValueId::of_bytes(b"x").as_str()), None);
    }

    #[test]
    fn reserved_words_and_malformed_strings_are_not_ids() {
        let ok = gen_().derive(IdKind::Handle, &["x"]);
        let too_long = format!("{ok}0{}", "0".repeat(format().max_length));
        for bad in [
            "self",
            "outside",
            "",
            "h-",
            "h",
            "h-ABC",
            "h-ab cd",
            "x-0123",
            "h:0123",
            too_long.as_str(),
        ] {
            assert_eq!(kind_of(bad), None, "{bad:?}");
        }
    }
}
