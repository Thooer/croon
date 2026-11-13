//! Value：规范文本、上限、ID。
//! 参考：docs/Kernel/Value.md。
//!
//! JSON Value 用 `serde_json::Value` 表示；字节 Value 就是 `[u8]`。两者的 ID 在同一个空间。

use std::fmt::Write as _;
use std::sync::OnceLock;

use serde_json::{Number, Value as Json};
use sha2::{Digest, Sha256};

/// 文档里 `value.limits` 的字段。
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub number_max: u64,
    pub depth: usize,
    pub json_bytes: usize,
    pub raw_bytes: usize,
}

impl Limits {
    pub fn get() -> &'static Limits {
        static LIMITS: OnceLock<Limits> = OnceLock::new();
        LIMITS.get_or_init(|| {
            let block = crate::blocks::json("value.limits");
            let field = |name: &str| {
                block
                    .get(name)
                    .and_then(Json::as_u64)
                    .unwrap_or_else(|| panic!("value.limits 缺少整数字段 {name}"))
            };
            Limits {
                number_max: field("number_max"),
                depth: field("depth") as usize,
                json_bytes: field("json_bytes") as usize,
                raw_bytes: field("raw_bytes") as usize,
            }
        })
    }
}

/// 不是 Value 的原因。`path` 是 JSON Pointer，根是空串。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValueError {
    #[error("{path}：数 {value} 的绝对值超过 {max}，不是 Value；要更大的整数，用字符串")]
    NumberTooLarge {
        path: String,
        value: String,
        max: u64,
    },
    #[error("{path}：嵌套深度超过 {max}，不是 Value")]
    TooDeep { path: String, max: usize },
    #[error("规范文本超过 {max} 字节，不是 Value")]
    JsonTooLarge { max: usize },
    #[error("字节 {size} 个，超过上限 {max}，不是 Value")]
    BytesTooLarge { size: usize, max: usize },
}

/// JSON Value 的规范文本：键按 UTF-16 码元排序、没有空白、数用 ECMAScript 的最短表示。
/// 同时检查 `value.limits`：超出的不是 Value。
pub fn canonical(value: &Json) -> Result<String, ValueError> {
    let limits = Limits::get();
    let mut out = String::new();
    let mut path = String::new();
    write_value(value, &mut out, &mut path, 0, limits)?;
    // 写入途中只查到「已写的部分」；最后一个值写完后还要再查一次。
    if out.len() > limits.json_bytes {
        return Err(ValueError::JsonTooLarge {
            max: limits.json_bytes,
        });
    }
    Ok(out)
}

fn write_value(
    value: &Json,
    out: &mut String,
    path: &mut String,
    depth: usize,
    limits: &Limits,
) -> Result<(), ValueError> {
    if out.len() > limits.json_bytes {
        return Err(ValueError::JsonTooLarge {
            max: limits.json_bytes,
        });
    }
    match value {
        Json::Null => out.push_str("null"),
        Json::Bool(true) => out.push_str("true"),
        Json::Bool(false) => out.push_str("false"),
        Json::Number(n) => out.push_str(&number_text(n, limits, path)?),
        Json::String(s) => write_string(s, out),
        Json::Array(items) => {
            enter(depth, path, limits)?;
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                let mark = path.len();
                write!(path, "/{i}").expect("写进 String 不会失败");
                write_value(item, out, path, depth + 1, limits)?;
                path.truncate(mark);
            }
            out.push(']');
        }
        Json::Object(map) => {
            enter(depth, path, limits)?;
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            out.push('{');
            for (i, key) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(key, out);
                out.push(':');
                let mark = path.len();
                path.push('/');
                path.push_str(&key.replace('~', "~0").replace('/', "~1"));
                write_value(&map[key], out, path, depth + 1, limits)?;
                path.truncate(mark);
            }
            out.push('}');
        }
    }
    Ok(())
}

fn enter(depth: usize, path: &str, limits: &Limits) -> Result<(), ValueError> {
    if depth + 1 > limits.depth {
        return Err(ValueError::TooDeep {
            path: path.to_owned(),
            max: limits.depth,
        });
    }
    Ok(())
}

/// 字符串的转义照 RFC 8785：只转义必须转义的，其余原样。
fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => {
                write!(out, "\\u{:04x}", c as u32).expect("写进 String 不会失败");
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn number_text(n: &Number, limits: &Limits, path: &str) -> Result<String, ValueError> {
    let too_large = |value: String| ValueError::NumberTooLarge {
        path: path.to_owned(),
        value,
        max: limits.number_max,
    };
    if let Some(i) = n.as_i64() {
        if i.unsigned_abs() > limits.number_max {
            return Err(too_large(i.to_string()));
        }
        return Ok(i.to_string());
    }
    if let Some(u) = n.as_u64() {
        if u > limits.number_max {
            return Err(too_large(u.to_string()));
        }
        return Ok(u.to_string());
    }
    // 其余都是浮点数：serde_json 不接受 NaN 与无穷，所以一定是有限的。
    let x = n.as_f64().expect("Number 总能取成 f64");
    if x.abs() > limits.number_max as f64 {
        return Err(too_large(es_number(x)));
    }
    Ok(es_number(x))
}

/// ECMAScript 的 Number::toString：最短的、能还原同一个 double 的十进制表示。
/// 只在 |x| ≤ 2^53−1 的范围里用到，所以指数形式只会出现在很小的数上。
fn es_number(x: f64) -> String {
    if x == 0.0 {
        return "0".to_owned(); // 负零也是 0
    }
    let sign = if x < 0.0 { "-" } else { "" };
    // `{:e}` 给出最短的往返表示，如 1.2345e-7。
    let sci = format!("{:e}", x.abs());
    let (mantissa, exp) = sci.split_once('e').expect("{:e} 总带指数");
    let exp: i32 = exp.parse().expect("指数是整数");
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i32;
    let n = exp + 1; // 值 = 0.digits × 10^n

    let body = if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let e = n - 1;
        let sign_e = if e < 0 { '-' } else { '+' };
        if k == 1 {
            format!("{digits}e{sign_e}{}", e.abs())
        } else {
            format!("{}.{}e{sign_e}{}", &digits[..1], &digits[1..], e.abs())
        }
    };
    format!("{sign}{body}")
}

/// Value 的 ID：`sha256:` 加十六进制。JSON 取规范文本的 UTF-8 字节，字节取字节本身。
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ValueId(String);

const PREFIX: &str = "sha256:";

impl ValueId {
    pub fn of_bytes(bytes: &[u8]) -> ValueId {
        let digest = Sha256::digest(bytes);
        let mut s = String::with_capacity(PREFIX.len() + 64);
        s.push_str(PREFIX);
        for b in digest {
            write!(s, "{b:02x}").expect("写进 String 不会失败");
        }
        ValueId(s)
    }

    pub fn of_json(value: &Json) -> Result<ValueId, ValueError> {
        Ok(ValueId::of_bytes(canonical(value)?.as_bytes()))
    }

    /// 认格式：`sha256:` 加 64 位小写十六进制。
    pub fn parse(s: &str) -> Option<ValueId> {
        let hex = s.strip_prefix(PREFIX)?;
        let ok = hex.len() == 64 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        ok.then(|| ValueId(s.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 十六进制部分，存储按它分目录。
    pub fn hex(&self) -> &str {
        &self.0[PREFIX.len()..]
    }
}

impl std::fmt::Display for ValueId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// 字节 Value 的大小检查。
pub fn check_bytes(bytes: &[u8]) -> Result<(), ValueError> {
    let max = Limits::get().raw_bytes;
    if bytes.len() > max {
        return Err(ValueError::BytesTooLarge {
            size: bytes.len(),
            max,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn canon(text: &str) -> Result<String, ValueError> {
        canonical(&serde_json::from_str(text).expect("测试里的 JSON 合法"))
    }

    #[test]
    fn object_keys_sort_and_whitespace_goes() {
        assert_eq!(
            canon(r#"{ "b": 1, "a": [ true, null ] }"#).unwrap(),
            r#"{"a":[true,null],"b":1}"#
        );
    }

    #[test]
    fn keys_sort_by_utf16_code_units_not_utf8_bytes() {
        // U+1F600 的 UTF-16 是 D83D DE00，小于 U+FB33 的 FB33；UTF-8 的字节序则相反。
        let text = canon(r#"{"\uFB33":1,"\ud83d\ude00":2,"\u20ac":3,"1":4,"\u0080":5}"#).unwrap();
        assert_eq!(
            text,
            "{\"1\":4,\"\u{80}\":5,\"\u{20ac}\":3,\"\u{1F600}\":2,\"\u{FB33}\":1}"
        );
    }

    #[test]
    fn strings_escape_only_what_must_be() {
        let v = Json::String("a\"b\\c\n\t\u{1}\u{7f}/é".to_owned());
        assert_eq!(
            canonical(&v).unwrap(),
            "\"a\\\"b\\\\c\\n\\t\\u0001\u{7f}/é\""
        );
    }

    #[test]
    fn numbers_use_the_shortest_ecmascript_form() {
        for (input, want) in [
            ("1.0", "1"),
            ("100", "100"),
            ("0.1", "0.1"),
            ("-2.5", "-2.5"),
            ("123.456", "123.456"),
            ("0.000001", "0.000001"),
            ("1e-7", "1e-7"),
            ("1.5e-7", "1.5e-7"),
            ("1e3", "1000"),
            ("5e-324", "5e-324"),
            ("-0", "0"),
            ("-0.0", "0"),
        ] {
            assert_eq!(canon(input).unwrap(), want, "输入 {input}");
        }
    }

    #[test]
    fn the_largest_safe_integer_is_a_value_and_the_next_is_not() {
        assert_eq!(canon("9007199254740991").unwrap(), "9007199254740991");
        assert_eq!(canon("-9007199254740991").unwrap(), "-9007199254740991");
        for text in [
            "9007199254740992",
            "9007199254740993",
            "-9007199254740992",
            "1e300",
            "12345678901234567890123",
        ] {
            assert!(
                matches!(canon(text), Err(ValueError::NumberTooLarge { .. })),
                "{text} 不该是 Value：{:?}",
                canon(text)
            );
        }
    }

    #[test]
    fn two_different_big_integers_never_share_an_id() {
        // 如果悄悄取整，这两个会得到同一个 ID。
        let a = ValueId::of_json(&serde_json::from_str("9007199254740992").unwrap());
        let b = ValueId::of_json(&serde_json::from_str("9007199254740993").unwrap());
        assert!(a.is_err() && b.is_err());
    }

    #[test]
    fn number_errors_point_at_the_path() {
        let err = canon(r#"{"a/b":[1,{"~":9007199254740993}]}"#).unwrap_err();
        match err {
            ValueError::NumberTooLarge { path, .. } => assert_eq!(path, "/a~1b/1/~0"),
            other => panic!("{other:?}"),
        }
    }

    fn nested(levels: usize) -> Json {
        let mut v = json!(0);
        for _ in 0..levels {
            v = json!([v]);
        }
        v
    }

    #[test]
    fn depth_counts_containers_and_scalars_are_zero() {
        let max = Limits::get().depth;
        assert!(canonical(&nested(max)).is_ok());
        assert!(matches!(
            canonical(&nested(max + 1)),
            Err(ValueError::TooDeep { .. })
        ));
        assert!(canonical(&json!(1)).is_ok());
    }

    #[test]
    fn oversized_json_is_not_a_value() {
        let big = Json::String("x".repeat(Limits::get().json_bytes));
        assert!(matches!(
            canonical(&big),
            Err(ValueError::JsonTooLarge { .. })
        ));
    }

    #[test]
    fn oversized_bytes_are_not_a_value() {
        assert!(check_bytes(&[0; 16]).is_ok());
        assert!(matches!(
            check_bytes(&vec![0u8; Limits::get().raw_bytes + 1]),
            Err(ValueError::BytesTooLarge { .. })
        ));
    }

    #[test]
    fn id_of_empty_bytes_is_the_known_sha256() {
        assert_eq!(
            ValueId::of_bytes(b"").as_str(),
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn json_and_bytes_share_one_id_space() {
        let v = json!({"b": 1, "a": 2});
        let text = canonical(&v).unwrap();
        assert_eq!(
            ValueId::of_json(&v).unwrap(),
            ValueId::of_bytes(text.as_bytes())
        );
    }

    #[test]
    fn equal_values_in_different_key_order_have_one_id() {
        let a = ValueId::of_json(&serde_json::from_str(r#"{"x":1,"y":[1.0]}"#).unwrap()).unwrap();
        let b = ValueId::of_json(&serde_json::from_str(r#"{"y":[1],"x":1}"#).unwrap()).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn id_parse_accepts_only_the_exact_format() {
        let id = ValueId::of_bytes(b"x");
        assert_eq!(ValueId::parse(id.as_str()), Some(id.clone()));
        assert_eq!(id.hex().len(), 64);
        for bad in [
            "",
            "sha256:",
            "sha256:ABC",
            "sha1:abcd",
            "h-0123",
            &format!("{}0", id.as_str()),
        ] {
            assert_eq!(ValueId::parse(bad), None, "{bad}");
        }
    }
}
