//! 记录库：Journal 的存储。
//! 参考：docs/Kernel/Journal.md（实现）。
//!
//! 记录之外只有宿主一侧的状态：代际号（只作 fencing）与停下的标记，都不是事实。
//!
//! 所有 SQL 都在这个文件里，只写标准 SQL：整数、文本，记录按规范文本存；代际号用带条件的
//! UPDATE，看影响的行数，不依赖隔离级别。以后换 PostgreSQL，换的是这一个文件。
//!
//! 记录以 JSON 出入，形状只在文档的 `instance.record` 里：每次写入都按它校验，Kernel 写的
//! 每一条记录因此都满足它。

use std::fmt::Write as _;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde_json::{Value as Json, json};

use crate::schema::{Violation, validate};
use crate::value::{self, ValueError};

#[derive(Debug, thiserror::Error)]
pub enum JournalError {
    #[error("记录库：{0}")]
    Sql(#[from] rusqlite::Error),
    #[error(transparent)]
    Value(#[from] ValueError),
    #[error("{instance} 的这条记录不满足 instance.record：{}", show(.violations))]
    Shape {
        instance: String,
        violations: Vec<Violation>,
    },
    #[error(
        "{0} has an unresolved host execution; collect or retire it before acquiring a new generation"
    )]
    PendingExecution(String),
    #[error("没有 Instance {0}")]
    Unknown(String),
    #[error("Instance {0} 已经存在")]
    Exists(String),
    #[error("诞生必须是 {0} 的第一条记录，什么是 birth")]
    NotBirth(String),
    #[error("{instance} 的键 `{key}` 已经有记录：同一个键只有一条记录")]
    DuplicateKey { instance: String, key: String },
    #[error("{instance} 的代际号是 {current}，{held} 已经过期，交出被拒")]
    Stale {
        instance: String,
        held: u64,
        current: u64,
    },
    #[error("{instance} 的第 {seq} 条记录读不出来：{reason}")]
    Corrupt {
        instance: String,
        seq: u64,
        reason: String,
    },
}

fn show(violations: &[Violation]) -> String {
    let mut out = String::new();
    for (i, v) in violations.iter().enumerate() {
        if i > 0 {
            out.push('；');
        }
        write!(out, "{v}").expect("写进 String 不会失败");
    }
    out
}

/// 要写入的一条记录；序号由记录库给。`at` 由 Kernel 记下，Language 看不到。
#[derive(Debug, Clone, PartialEq)]
pub struct NewRecord {
    pub at: String,
    pub by: String,
    pub event: Option<String>,
    pub what: Json,
}

/// 一个 Instance 在记录库里的位置：代际号与已有的记录数（也是下一条的序号）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    pub generation: u64,
    pub len: u64,
}

pub struct Journal {
    conn: Connection,
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS instances (
    id TEXT NOT NULL PRIMARY KEY,
    generation INTEGER NOT NULL,
    next_seq INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS records (
    instance TEXT NOT NULL,
    seq INTEGER NOT NULL,
    key TEXT,
    body TEXT NOT NULL,
    PRIMARY KEY (instance, seq),
    UNIQUE (instance, key)
);
CREATE TABLE IF NOT EXISTS attempt_counter (id INTEGER PRIMARY KEY AUTOINCREMENT);
CREATE TABLE IF NOT EXISTS host_attempts (attempt TEXT PRIMARY KEY, instance TEXT NOT NULL, atom_key TEXT NOT NULL, committed INTEGER NOT NULL, snapshot TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS stopped (
    instance TEXT NOT NULL PRIMARY KEY,
    reason TEXT NOT NULL,
    world_effect INTEGER NOT NULL DEFAULT 0
);
";

impl Journal {
    pub fn open(path: impl AsRef<Path>) -> Result<Journal, JournalError> {
        Journal::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Journal, JournalError> {
        Journal::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Journal, JournalError> {
        // 显式设定，不依赖默认值：崩溃后重新打开即恢复，提交时落盘。
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Journal { conn })
    }

    pub fn next_attempt(&self) -> Result<u64, JournalError> {
        self.conn
            .execute("INSERT INTO attempt_counter DEFAULT VALUES", [])?;
        Ok(self.conn.last_insert_rowid() as u64)
    }
    pub fn accept(
        &self,
        attempt: &str,
        instance: &str,
        key: &str,
        snapshot: &Json,
    ) -> Result<(), JournalError> {
        let text = value::canonical(snapshot)?;
        let existing: Option<(String, String, String)> = self
            .conn
            .query_row(
                "SELECT instance,atom_key,snapshot FROM host_attempts WHERE attempt=?1",
                params![attempt],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        if let Some((owner, atom_key, stored)) = existing {
            if owner == instance && atom_key == key && stored == text {
                return Ok(());
            }
            return Err(JournalError::Corrupt {
                instance: instance.into(),
                seq: 0,
                reason: "attempt identity reused with different acceptance snapshot".into(),
            });
        }
        let held = snapshot["generation"].as_u64().unwrap_or(0);
        let inserted=self.conn.execute(
            "INSERT INTO host_attempts SELECT ?1,?2,?3,0,?4 FROM instances WHERE id=?2 AND generation=?5",
            params![attempt, instance, key, text,to_i64(held)],
        )?;
        if inserted == 0 {
            let state = self
                .state(instance)?
                .ok_or_else(|| JournalError::Unknown(instance.into()))?;
            return Err(JournalError::Stale {
                instance: instance.into(),
                held,
                current: state.generation,
            });
        }
        Ok(())
    }
    pub fn replace_attempt(
        &mut self,
        old: &str,
        new: &str,
        instance: &str,
        key: &str,
        snapshot: &Json,
    ) -> Result<(), JournalError> {
        let text = value::canonical(snapshot)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute("DELETE FROM host_attempts WHERE attempt=?1", params![old])?;
        tx.execute(
            "INSERT INTO host_attempts VALUES (?1,?2,?3,0,?4)",
            params![new, instance, key, text],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn attempts(&self) -> Result<Vec<(String, String, String, bool, Json)>, JournalError> {
        let mut stmt=self.conn.prepare("SELECT attempt,instance,atom_key,committed,snapshot FROM host_attempts ORDER BY attempt")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, bool>(3)?,
                r.get::<_, String>(4)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (a, i, k, c, s) = row?;
            let json = serde_json::from_str(&s).map_err(|e| JournalError::Corrupt {
                instance: i.clone(),
                seq: 0,
                reason: e.to_string(),
            })?;
            out.push((a, i, k, c, json));
        }
        Ok(out)
    }
    pub fn retire_attempt(&self, attempt: &str) -> Result<(), JournalError> {
        self.conn.execute(
            "DELETE FROM host_attempts WHERE attempt=?1",
            params![attempt],
        )?;
        Ok(())
    }

    /// 在一个事务里做一批写入：闭包返回 Err 或 panic，全部不留下；返回 Ok 才提交。
    /// 交出与它造成的到达、撤销造成的全部失效、Collective 与成员的诞生，各是一个事务。
    pub fn transaction<T>(
        &mut self,
        f: impl FnOnce(&mut Tx<'_>) -> Result<T, JournalError>,
    ) -> Result<T, JournalError> {
        // 一开始就拿写锁：先读后写的事务在别的连接写过之后升级会失败。
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut tx = Tx { tx };
        let out = f(&mut tx)?;
        tx.tx.commit()?;
        Ok(out)
    }

    pub fn state(&self, instance: &str) -> Result<Option<State>, JournalError> {
        state_of(&self.conn, instance)
    }

    pub fn instances(&self) -> Result<Vec<String>, JournalError> {
        instances_of(&self.conn)
    }

    /// 从序号 `from` 起读至多 `limit` 条，按序号。
    pub fn read(&self, instance: &str, from: u64, limit: usize) -> Result<Vec<Json>, JournalError> {
        read_of(&self.conn, instance, from, limit)
    }

    /// 这个键的那一条记录；没有就是还没求值过。
    pub fn find_key(&self, instance: &str, key: &str) -> Result<Option<Json>, JournalError> {
        find_key_of(&self.conn, instance, key)
    }

    /// 全部 Instance 的全部记录，各自按序号：重建索引用。
    pub fn all(&self) -> Result<Vec<(String, Vec<Json>)>, JournalError> {
        let mut out = Vec::new();
        for instance in self.instances()? {
            let records = self.read(&instance, 0, usize::MAX)?;
            out.push((instance, records));
        }
        Ok(out)
    }

    /// 停下一个 Instance：重放分歧或同一处反复崩溃。要撑过重启，恢复时不重开它。
    pub fn stop(
        &mut self,
        instance: &str,
        reason: &str,
        world_effect: bool,
    ) -> Result<(), JournalError> {
        self.conn.execute(
            "INSERT INTO stopped (instance, reason, world_effect) VALUES (?1, ?2, ?3) ON CONFLICT (instance) DO UPDATE SET reason = excluded.reason, world_effect = CASE WHEN stopped.world_effect <> 0 THEN stopped.world_effect ELSE excluded.world_effect END",
            params![instance, reason, world_effect],
        )?;
        Ok(())
    }

    /// 清除宿主停止标记；新的代际由下一次 poll 取得。
    pub fn resume_stopped(&mut self, instance: &str) -> Result<(), JournalError> {
        self.conn
            .execute("DELETE FROM stopped WHERE instance = ?1", params![instance])?;
        Ok(())
    }

    pub fn set_stopped_world_effect(
        &mut self,
        instance: &str,
        pending: bool,
    ) -> Result<(), JournalError> {
        self.conn.execute(
            "UPDATE stopped SET world_effect = ?2 WHERE instance = ?1",
            params![instance, pending],
        )?;
        Ok(())
    }

    /// 被停下的 Instance 与原因。
    pub fn stopped(&self) -> Result<Vec<(String, String, bool)>, JournalError> {
        let mut stmt = self
            .conn
            .prepare("SELECT instance, reason, world_effect FROM stopped ORDER BY instance")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get::<_, i64>(2)? != 0))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}

pub struct Tx<'a> {
    tx: Transaction<'a>,
}

impl Tx<'_> {
    /// 产生一个 Instance：写下它的诞生，那是第一条记录。代际号从 0 起，还没有代际取得执行权。
    pub fn create(&mut self, instance: &str, birth: NewRecord) -> Result<(), JournalError> {
        if birth.what.get("birth").is_none() {
            return Err(JournalError::NotBirth(instance.to_owned()));
        }
        if state_of(&self.tx, instance)?.is_some() {
            return Err(JournalError::Exists(instance.to_owned()));
        }
        // 先校验、再写：被拒的诞生不留下一个没有记录的 Instance。
        let built = build(instance, 0, &birth)?;
        self.tx.execute(
            "INSERT INTO instances (id, generation, next_seq) VALUES (?1, 0, 0)",
            params![instance],
        )?;
        self.insert(instance, None, built)?;
        Ok(())
    }

    /// 取得执行权：代际号加一，旧代际的交出从此被拒。
    pub fn acquire(&mut self, instance: &str) -> Result<u64, JournalError> {
        let pending: bool = self.tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM host_attempts WHERE instance=?1)",
            params![instance],
            |r| r.get(0),
        )?;
        if pending {
            return Err(JournalError::PendingExecution(instance.into()));
        }
        let generation: Option<i64> = self
            .tx
            .query_row(
                "UPDATE instances SET generation = generation + 1 WHERE id = ?1 RETURNING generation",
                params![instance],
                |row| row.get(0),
            )
            .optional()?;
        generation
            .map(to_u64)
            .ok_or_else(|| JournalError::Unknown(instance.to_owned()))
    }

    /// 自己的一步（交出、取、结束）：必须带着自己持有的代际号，过期的被拒。
    pub fn append_step(
        &mut self,
        instance: &str,
        generation: u64,
        record: NewRecord,
    ) -> Result<u64, JournalError> {
        self.append(instance, Some(generation), record)
    }

    /// 别人造成的记录（到达、控制、失效），以及自己的一步造成的、记在同一个事务里的记录：
    /// 不看代际号，是那一步已经过了代际号的检查。
    pub fn append_caused(
        &mut self,
        instance: &str,
        record: NewRecord,
    ) -> Result<u64, JournalError> {
        self.append(instance, None, record)
    }

    fn append(
        &mut self,
        instance: &str,
        fence: Option<u64>,
        record: NewRecord,
    ) -> Result<u64, JournalError> {
        // 先分类，再动手：下面每一个错误都发生在写入之前，失败的写入不占序号。
        let state = state_of(&self.tx, instance)?
            .ok_or_else(|| JournalError::Unknown(instance.to_owned()))?;
        if let Some(held) = fence
            && held != state.generation
        {
            return Err(JournalError::Stale {
                instance: instance.to_owned(),
                held,
                current: state.generation,
            });
        }
        let built = build(instance, state.len, &record)?;
        if let Some(key) = &built.key
            && find_key_of(&self.tx, instance, key)?.is_some()
        {
            return Err(JournalError::DuplicateKey {
                instance: instance.to_owned(),
                key: key.clone(),
            });
        }
        self.insert(instance, fence, built)
    }

    fn insert(
        &mut self,
        instance: &str,
        fence: Option<u64>,
        built: Built,
    ) -> Result<u64, JournalError> {
        // 序号与代际号一起在一条带条件的 UPDATE 里：条件不成立就一行也不动。
        let taken: Option<i64> = self
            .tx
            .query_row(
                "UPDATE instances SET next_seq = next_seq + 1 \
                 WHERE id = ?1 AND (?2 IS NULL OR generation = ?2) RETURNING next_seq - 1",
                params![instance, fence.map(to_i64)],
                |row| row.get(0),
            )
            .optional()?;
        let seq = match taken {
            Some(seq) => to_u64(seq),
            // 分类时代际号还对得上，这时对不上了：别的执行者在这之间取得了执行权。
            None => {
                let current = state_of(&self.tx, instance)?
                    .ok_or_else(|| JournalError::Unknown(instance.to_owned()))?;
                return Err(JournalError::Stale {
                    instance: instance.to_owned(),
                    held: fence.unwrap_or(current.generation),
                    current: current.generation,
                });
            }
        };
        if seq != built.seq {
            return Err(JournalError::Corrupt {
                instance: instance.to_owned(),
                seq,
                reason: format!("分配到的序号与校验时的 {} 对不上", built.seq),
            });
        }
        if let Some(key) = &built.key {
            self.tx.execute(
                "UPDATE host_attempts SET committed=1 WHERE instance=?1 AND atom_key=?2",
                params![instance, key],
            )?;
        }
        self.tx.execute(
            "INSERT INTO records (instance, seq, key, body) VALUES (?1, ?2, ?3, ?4)",
            params![instance, to_i64(seq), built.key, built.body],
        )?;
        Ok(seq)
    }

    pub fn state(&self, instance: &str) -> Result<Option<State>, JournalError> {
        state_of(&self.tx, instance)
    }

    pub fn read(&self, instance: &str, from: u64, limit: usize) -> Result<Vec<Json>, JournalError> {
        read_of(&self.tx, instance, from, limit)
    }

    pub fn find_key(&self, instance: &str, key: &str) -> Result<Option<Json>, JournalError> {
        find_key_of(&self.tx, instance, key)
    }
}

/// 校验过、可以写入的一条记录。
struct Built {
    seq: u64,
    key: Option<String>,
    body: String,
}

/// 纯函数：拼出完整的记录，按 instance.record 校验，取出键与规范文本。
fn build(instance: &str, seq: u64, record: &NewRecord) -> Result<Built, JournalError> {
    let mut full = json!({
        "seq": seq,
        "at": record.at,
        "by": record.by,
        "what": record.what,
    });
    if let Some(event) = &record.event {
        full["event"] = json!(event);
    }
    let schema = crate::blocks::json("instance.record");
    validate(&schema, &full).map_err(|violations| JournalError::Shape {
        instance: instance.to_owned(),
        violations,
    })?;
    // 自己的一步有键：交出与取共用一个键的空间。
    let key = ["send", "receive"]
        .iter()
        .find_map(|kind| record.what.get(kind)?.get("key")?.as_str())
        .map(str::to_owned);
    Ok(Built {
        seq,
        key,
        // 规范文本：同样的记录写出同样的字节。
        body: value::canonical(&full)?,
    })
}

fn to_i64(n: u64) -> i64 {
    i64::try_from(n).expect("序号与代际号不会超过 i64")
}

fn to_u64(n: i64) -> u64 {
    u64::try_from(n).expect("库里的序号与代际号不是负数")
}

fn state_of(conn: &Connection, instance: &str) -> Result<Option<State>, JournalError> {
    Ok(conn
        .query_row(
            "SELECT generation, next_seq FROM instances WHERE id = ?1",
            params![instance],
            |row| {
                Ok(State {
                    generation: to_u64(row.get(0)?),
                    len: to_u64(row.get(1)?),
                })
            },
        )
        .optional()?)
}

fn instances_of(conn: &Connection) -> Result<Vec<String>, JournalError> {
    let mut stmt = conn.prepare("SELECT id FROM instances ORDER BY id")?;
    let ids = stmt.query_map([], |row| row.get(0))?;
    Ok(ids.collect::<Result<_, _>>()?)
}

fn parse(instance: &str, seq: u64, body: &str) -> Result<Json, JournalError> {
    serde_json::from_str(body).map_err(|e| JournalError::Corrupt {
        instance: instance.to_owned(),
        seq,
        reason: e.to_string(),
    })
}

fn read_of(
    conn: &Connection,
    instance: &str,
    from: u64,
    limit: usize,
) -> Result<Vec<Json>, JournalError> {
    if state_of(conn, instance)?.is_none() {
        return Err(JournalError::Unknown(instance.to_owned()));
    }
    let mut stmt = conn.prepare(
        "SELECT seq, body FROM records WHERE instance = ?1 AND seq >= ?2 ORDER BY seq LIMIT ?3",
    )?;
    let rows = stmt.query_map(
        params![
            instance,
            to_i64(from),
            i64::try_from(limit).unwrap_or(i64::MAX)
        ],
        |row| Ok((to_u64(row.get(0)?), row.get::<_, String>(1)?)),
    )?;
    rows.map(|row| {
        let (seq, body) = row?;
        parse(instance, seq, &body)
    })
    .collect()
}

fn find_key_of(conn: &Connection, instance: &str, key: &str) -> Result<Option<Json>, JournalError> {
    let row: Option<(i64, String)> = conn
        .query_row(
            "SELECT seq, body FROM records WHERE instance = ?1 AND key = ?2",
            params![instance, key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    row.map(|(seq, body)| parse(instance, to_u64(seq), &body))
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::TempDir;
    use crate::value::ValueId;

    fn id() -> String {
        ValueId::of_bytes(b"x").to_string()
    }

    fn record(by: &str, event: Option<&str>, what: Json) -> NewRecord {
        NewRecord {
            at: "2026-01-01T00:00:00Z".to_owned(),
            by: by.to_owned(),
            event: event.map(str::to_owned),
            what,
        }
    }

    fn birth() -> NewRecord {
        record(
            "outside",
            Some("e-1"),
            json!({"birth": {"definition": id(), "input":{"value":id(),"dependencies":[]}, "wire": {}}}),
        )
    }

    fn send(key: &str) -> NewRecord {
        record(
            "self",
            None,
            json!({"send": {"key": key, "to": {"port": "engine"}, "ref": "h-1", "delivery":{"value":id(),"dependencies":[]}, "attach": {}, "result": {"outcome": {"ok": true, "value": null,"dependencies":[]}, "attach": {}, "derived": {},"accepted":true}}}),
        )
    }

    fn receive(key: &str) -> NewRecord {
        record(
            "self",
            None,
            json!({"receive": {"key": key, "filter": {"ports": ["inbox"]}, "result": null}}),
        )
    }

    fn arrival() -> NewRecord {
        record(
            "i-other",
            Some("e-2"),
            json!({"arrival": {"port": "inbox", "sender": "i-other", "via": "h-2", "delivery":{"value":id(),"dependencies":[]}, "attach": {}}}),
        )
    }

    fn end() -> NewRecord {
        record(
            "self",
            None,
            json!({"end": {"ok": true, "value": null,"dependencies":[]}}),
        )
    }

    /// 一本带诞生的 Journal，里面有 Instance `i-a`。
    fn with_a() -> Journal {
        let mut j = Journal::open_in_memory().unwrap();
        j.transaction(|tx| tx.create("i-a", birth())).unwrap();
        j
    }

    fn len(j: &Journal, instance: &str) -> u64 {
        j.state(instance).unwrap().unwrap().len
    }

    fn raw_body(j: &Journal, instance: &str, seq: u64) -> String {
        j.conn
            .query_row(
                "SELECT body FROM records WHERE instance = ?1 AND seq = ?2",
                params![instance, to_i64(seq)],
                |r| r.get(0),
            )
            .unwrap()
    }

    #[test]
    fn birth_is_the_first_record_and_the_instance_starts_without_an_executor() {
        let j = with_a();
        assert_eq!(
            j.state("i-a").unwrap(),
            Some(State {
                generation: 0,
                len: 1
            })
        );
        let records = j.read("i-a", 0, 10).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["seq"], 0);
        assert!(records[0]["what"].get("birth").is_some());
        assert_eq!(j.instances().unwrap(), vec!["i-a".to_owned()]);
    }

    #[test]
    fn an_instance_is_created_once_and_its_first_record_must_be_a_birth() {
        let mut j = with_a();
        let again = j.transaction(|tx| tx.create("i-a", birth()));
        assert!(matches!(again, Err(JournalError::Exists(_))), "{again:?}");
        let not_birth = j.transaction(|tx| tx.create("i-b", end()));
        assert!(
            matches!(not_birth, Err(JournalError::NotBirth(_))),
            "{not_birth:?}"
        );
        assert_eq!(j.state("i-b").unwrap(), None);
    }

    #[test]
    fn a_rejected_birth_leaves_no_instance_behind_even_if_the_caller_carries_on() {
        let mut j = Journal::open_in_memory().unwrap();
        let bad = record("outside", None, json!({"birth": {"definition": 1}}));
        j.transaction(|tx| {
            assert!(matches!(
                tx.create("i-x", bad),
                Err(JournalError::Shape { .. })
            ));
            Ok(())
        })
        .unwrap();
        assert_eq!(j.state("i-x").unwrap(), None);
    }

    #[test]
    fn sequence_numbers_are_gapless_and_per_instance() {
        let mut j = with_a();
        j.transaction(|tx| {
            tx.create("i-b", birth())?;
            assert_eq!(tx.append_caused("i-a", arrival())?, 1);
            assert_eq!(tx.append_caused("i-b", arrival())?, 1);
            assert_eq!(tx.append_caused("i-a", arrival())?, 2);
            Ok(())
        })
        .unwrap();
        assert_eq!(len(&j, "i-a"), 3);
        assert_eq!(len(&j, "i-b"), 2);
        let seqs: Vec<u64> = j
            .read("i-a", 0, 10)
            .unwrap()
            .iter()
            .map(|r| r["seq"].as_u64().unwrap())
            .collect();
        assert_eq!(seqs, vec![0, 1, 2]);
    }

    #[test]
    fn read_honours_from_and_limit_and_refuses_unknown_instances() {
        let mut j = with_a();
        j.transaction(|tx| {
            for _ in 0..4 {
                tx.append_caused("i-a", arrival())?;
            }
            Ok(())
        })
        .unwrap();
        let seqs = |from, limit| -> Vec<u64> {
            j.read("i-a", from, limit)
                .unwrap()
                .iter()
                .map(|r| r["seq"].as_u64().unwrap())
                .collect()
        };
        assert_eq!(seqs(2, 10), vec![2, 3, 4]);
        assert_eq!(seqs(1, 2), vec![1, 2]);
        assert_eq!(seqs(9, 10), Vec::<u64>::new());
        assert_eq!(seqs(0, 0), Vec::<u64>::new());
        assert!(matches!(
            j.read("nobody", 0, 1),
            Err(JournalError::Unknown(_))
        ));
    }

    #[test]
    fn a_key_has_one_record_whether_it_is_a_send_or_a_receive() {
        let mut j = with_a();
        let g = j.acquire_for_test("i-a");
        j.transaction(|tx| tx.append_step("i-a", g, send("0/0")))
            .unwrap();
        let before = j.state("i-a").unwrap();

        let dup_send = j.transaction(|tx| tx.append_step("i-a", g, send("0/0")));
        assert!(
            matches!(dup_send, Err(JournalError::DuplicateKey { .. })),
            "{dup_send:?}"
        );
        let dup_receive = j.transaction(|tx| tx.append_step("i-a", g, receive("0/0")));
        assert!(
            matches!(dup_receive, Err(JournalError::DuplicateKey { .. })),
            "{dup_receive:?}"
        );

        assert_eq!(j.state("i-a").unwrap(), before, "被拒的写入不占序号");
        let found = j.find_key("i-a", "0/0").unwrap().unwrap();
        assert!(found["what"].get("send").is_some());
        assert_eq!(j.find_key("i-a", "0/1").unwrap(), None);
    }

    #[test]
    fn records_without_a_key_never_collide() {
        let mut j = with_a();
        j.transaction(|tx| {
            tx.append_caused("i-a", arrival())?;
            tx.append_caused("i-a", arrival())?;
            Ok(())
        })
        .unwrap();
        assert_eq!(len(&j, "i-a"), 3);
    }

    #[test]
    fn an_old_generation_cannot_write_and_leaves_nothing_behind() {
        let mut j = with_a();
        let old = j.acquire_for_test("i-a");
        let new = j.acquire_for_test("i-a");
        assert_eq!((old, new), (1, 2));
        let before = j.state("i-a").unwrap();

        let refused = j.transaction(|tx| tx.append_step("i-a", old, send("0")));
        assert!(
            matches!(
                refused,
                Err(JournalError::Stale {
                    held: 1,
                    current: 2,
                    ..
                })
            ),
            "{refused:?}"
        );
        assert_eq!(j.state("i-a").unwrap(), before);
        assert_eq!(j.find_key("i-a", "0").unwrap(), None);

        j.transaction(|tx| tx.append_step("i-a", new, send("0")))
            .unwrap();
    }

    #[test]
    fn a_stale_executor_is_told_so_even_when_its_key_is_also_taken() {
        let mut j = with_a();
        let old = j.acquire_for_test("i-a");
        j.transaction(|tx| tx.append_step("i-a", old, send("0")))
            .unwrap();
        let new = j.acquire_for_test("i-a");
        let _ = new;
        let refused = j.transaction(|tx| tx.append_step("i-a", old, send("0")));
        assert!(
            matches!(refused, Err(JournalError::Stale { .. })),
            "{refused:?}"
        );
    }

    #[test]
    fn caused_records_do_not_need_the_executors_generation() {
        let mut j = with_a();
        j.acquire_for_test("i-a");
        j.acquire_for_test("i-a");
        j.transaction(|tx| tx.append_caused("i-a", arrival()))
            .unwrap();
        assert_eq!(len(&j, "i-a"), 2);
    }

    #[test]
    fn a_send_and_the_arrival_it_causes_are_written_together_or_not_at_all() {
        let mut j = with_a();
        j.transaction(|tx| tx.create("i-b", birth())).unwrap();
        let old = j.acquire_for_test("i-a");
        j.acquire_for_test("i-a");
        let (a, b) = (len(&j, "i-a"), len(&j, "i-b"));

        // 旧代际的交出被拒，它造成的到达也就没有写下。
        let refused = j.transaction(|tx| {
            tx.append_caused("i-b", arrival())?;
            tx.append_step("i-a", old, send("0"))?;
            Ok(())
        });
        assert!(matches!(refused, Err(JournalError::Stale { .. })));
        assert_eq!(
            (len(&j, "i-a"), len(&j, "i-b")),
            (a, b),
            "交出恰好一次：到达不能先于被拒的交出落下"
        );

        let current = j.state("i-a").unwrap().unwrap().generation;
        j.transaction(|tx| {
            tx.append_step("i-a", current, send("0"))?;
            tx.append_caused("i-b", arrival())?;
            Ok(())
        })
        .unwrap();
        assert_eq!((len(&j, "i-a"), len(&j, "i-b")), (a + 1, b + 1));
    }

    #[test]
    fn an_error_rolls_the_whole_transaction_back() {
        let mut j = with_a();
        let before = j.state("i-a").unwrap();
        let out: Result<(), _> = j.transaction(|tx| {
            tx.append_caused("i-a", arrival())?;
            tx.append_caused("i-a", arrival())?;
            tx.append_caused("nobody", arrival())?;
            Ok(())
        });
        assert!(matches!(out, Err(JournalError::Unknown(_))));
        assert_eq!(j.state("i-a").unwrap(), before);
    }

    #[test]
    fn a_panic_rolls_the_transaction_back_too() {
        let mut j = with_a();
        let before = j.state("i-a").unwrap();
        let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            j.transaction::<()>(|tx| {
                tx.append_caused("i-a", arrival())?;
                panic!("进程在提交之前崩溃");
            })
        }));
        assert!(out.is_err());
        assert_eq!(j.state("i-a").unwrap(), before);
    }

    #[test]
    fn a_failed_write_does_not_burn_a_sequence_number_inside_a_transaction() {
        let mut j = with_a();
        let g = j.acquire_for_test("i-a");
        j.transaction(|tx| {
            tx.append_step("i-a", g, send("0"))?;
            // 调用方接住错误、接着写：序号不能出现空洞。
            assert!(matches!(
                tx.append_step("i-a", g, send("0")),
                Err(JournalError::DuplicateKey { .. })
            ));
            assert!(matches!(
                tx.append_caused("i-a", record("self", None, json!({"invalid": "h-1"}))),
                Err(JournalError::Shape { .. })
            ));
            assert_eq!(tx.append_step("i-a", g, receive("1"))?, 2);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn records_that_do_not_satisfy_the_doc_schema_are_refused_with_a_path() {
        let mut j = with_a();
        let bad = [
            record(
                "self",
                None,
                json!({"end": {"ok": true, "value": null,"dependencies":[]}, "invalid": []}),
            ),
            record("self", None, json!({"nothing": 1})),
            record("self", None, json!({"send": {"key": "0"}})),
            record("i-b", Some(""), json!({"invalid": "h-1"})),
        ];
        for b in bad {
            let r = j.transaction(|tx| tx.append_caused("i-a", b.clone()));
            match r {
                Err(JournalError::Shape { violations, .. }) => assert!(!violations.is_empty()),
                other => panic!("{b:?} 应当被拒：{other:?}"),
            }
        }
        assert_eq!(len(&j, "i-a"), 1);
        let err = j
            .transaction(|tx| {
                tx.append_caused("i-a", record("self", None, json!({"invalid": "h-1"})))
            })
            .unwrap_err()
            .to_string();
        assert!(err.contains("/what"), "{err}");
    }

    #[test]
    fn the_stored_text_is_canonical_so_equal_records_are_equal_bytes() {
        let mut j = with_a();
        j.transaction(|tx| {
            tx.append_caused(
                "i-a",
                record("i-b", Some("e-2"), json!({"invalid": ["h-1", "h-2"]})),
            )?;
            tx.append_caused(
                "i-a",
                record(
                    "i-b",
                    Some("e-2"),
                    serde_json::from_str(r#"{ "invalid": ["h-1","h-2"] }"#).unwrap(),
                ),
            )?;
            Ok(())
        })
        .unwrap();
        let (a, b) = (raw_body(&j, "i-a", 1), raw_body(&j, "i-a", 2));
        assert_eq!(
            a.replace("\"seq\":1", "\"seq\":N"),
            b.replace("\"seq\":2", "\"seq\":N")
        );
        assert!(a.starts_with("{\"at\":"), "键按序排列，没有空白：{a}");
    }

    #[test]
    fn what_was_committed_survives_a_reopen_and_what_was_not_does_not() {
        let dir = TempDir::new("journal");
        let path = dir.0.join("journal.db");
        {
            let mut j = Journal::open(&path).unwrap();
            j.transaction(|tx| tx.create("i-a", birth())).unwrap();
            let g = j.transaction(|tx| tx.acquire("i-a")).unwrap();
            j.transaction(|tx| tx.append_step("i-a", g, send("0")))
                .unwrap();
            let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                j.transaction::<()>(|tx| {
                    tx.append_step("i-a", g, send("1"))?;
                    panic!("提交之前崩溃");
                })
            }));
            assert!(crashed.is_err());
        }
        let j = Journal::open(&path).unwrap();
        assert_eq!(
            j.state("i-a").unwrap(),
            Some(State {
                generation: 1,
                len: 2
            })
        );
        assert!(j.find_key("i-a", "0").unwrap().is_some());
        assert_eq!(j.find_key("i-a", "1").unwrap(), None);
    }

    #[test]
    fn durability_settings_are_explicit() {
        let dir = TempDir::new("pragma");
        let j = Journal::open(dir.0.join("j.db")).unwrap();
        let pragma = |name: &str| -> String {
            j.conn
                .query_row(&format!("PRAGMA {name}"), [], |r| {
                    r.get::<_, rusqlite::types::Value>(0)
                })
                .map(|v| match v {
                    rusqlite::types::Value::Text(s) => s,
                    rusqlite::types::Value::Integer(i) => i.to_string(),
                    other => format!("{other:?}"),
                })
                .unwrap()
        };
        assert_eq!(pragma("journal_mode"), "wal");
        assert_eq!(pragma("synchronous"), "2", "2 是 FULL");
    }

    impl Journal {
        /// 测试里取得执行权。
        fn acquire_for_test(&mut self, instance: &str) -> u64 {
            self.transaction(|tx| tx.acquire(instance)).unwrap()
        }
    }
    #[test]
    fn durable_acceptance_is_idempotent_and_fenced_before_start() {
        let mut journal = with_a();
        let generation = journal.transaction(|tx| tx.acquire("i-a")).unwrap();
        let snapshot = json!({"generation":generation,"request":"fixed"});
        journal.accept("attempt", "i-a", "call", &snapshot).unwrap();
        journal.accept("attempt", "i-a", "call", &snapshot).unwrap();
        assert_eq!(journal.attempts().unwrap().len(), 1);
        assert!(
            journal
                .accept(
                    "attempt",
                    "i-a",
                    "call",
                    &json!({"generation":generation,"request":"different"})
                )
                .is_err()
        );
        assert!(matches!(
            journal.transaction(|tx| tx.acquire("i-a")),
            Err(JournalError::PendingExecution(_))
        ));
        journal.retire_attempt("attempt").unwrap();
        journal.transaction(|tx| tx.acquire("i-a")).unwrap();
        assert!(matches!(
            journal.accept("stale", "i-a", "call", &snapshot),
            Err(JournalError::Stale { .. })
        ));
        assert!(journal.attempts().unwrap().is_empty());
    }
}
