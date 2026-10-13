//! Shared local usage ceiling for a subscription. No monetary estimate is made.
use crate::Result;
use rusqlite::{Connection, params};
use std::path::Path;
pub const RESERVATION: u64 = 26_624;
pub const DAILY_TOKENS: u64 = 1_000_000;
pub const DAILY_CALLS: u64 = 100;
pub struct Budget {
    db: Connection,
}
impl Budget {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let db = Connection::open(path).map_err(|_| "会员模型用量记录不可打开")?;
        db.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|_| "会员模型用量记录正在使用")?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS calls(id TEXT PRIMARY KEY,day INTEGER NOT NULL,reserved INTEGER NOT NULL,input INTEGER,output INTEGER);").map_err(|_| "会员模型用量记录不可建立")?;
        Ok(Self { db })
    }
    pub fn reserve(&mut self, clock: u64) -> Result<String> {
        let day = (clock / 86400) as i64;
        let tx = self
            .db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|_| "会员模型用量记录不可锁定")?;
        let (calls, used): (i64,i64) = tx.query_row("SELECT COUNT(*),COALESCE(SUM(COALESCE(input+output,reserved)),0) FROM calls WHERE day=?1", [day], |r| Ok((r.get(0)?,r.get(1)?))).map_err(|_| "会员模型用量记录不可读取")?;
        if calls >= DAILY_CALLS as i64
            || used
                .checked_add(RESERVATION as i64)
                .is_none_or(|n| n > DAILY_TOKENS as i64)
        {
            return Err("本机会员模型用量已达上限，手动功能仍可使用".into());
        }
        let id = action_receipts::new_id();
        tx.execute(
            "INSERT INTO calls(id,day,reserved) VALUES(?1,?2,?3)",
            params![id, day, RESERVATION as i64],
        )
        .map_err(|_| "会员模型调用预留不可保存")?;
        tx.commit().map_err(|_| "会员模型调用预留不可保存")?;
        Ok(id)
    }
    pub fn record(&self, id: &str, input: u64, output: u64) -> Result<()> {
        if input > 18_432 || output > 8192 {
            return Err("会员模型回传用量超出约定，保留调用预留".into());
        }
        if self.db.execute("UPDATE calls SET input=?2,output=?3 WHERE id=?1 AND input IS NULL AND output IS NULL",params![id,input as i64,output as i64]).map_err(|_| "会员模型用量不可保存")? != 1 {
            return Err("会员模型调用编号不存在或已记录".into());
        }
        Ok(())
    }
    pub fn summary(&self) -> Result<serde_json::Value> {
        let (calls,input,output,pending): (i64,i64,i64,i64) = self.db.query_row("SELECT COUNT(*),COALESCE(SUM(input),0),COALESCE(SUM(output),0),COALESCE(SUM(CASE WHEN input IS NULL THEN reserved ELSE 0 END),0) FROM calls",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).map_err(|_| "会员模型用量不可读取")?;
        Ok(
            serde_json::json!({"provider":"kimi-code","model":"k3","attempts_reserved":calls,"input_tokens":input,"output_tokens":output,"pending_reserved_tokens":pending,"billing_kind":"subscription","estimated_rmb":null,"remaining_plan_quota_verified":false}),
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "buwei-subscription-{}.db",
            action_receipts::new_id()
        ))
    }
    #[test]
    fn two_profiles_share_the_limit_and_uncertain_calls_keep_their_reservation() {
        let path = path();
        let mut a = Budget::open(&path).unwrap();
        let mut b = Budget::open(&path).unwrap();
        let mut count = 0;
        loop {
            let r = if count % 2 == 0 {
                a.reserve(10)
            } else {
                b.reserve(10)
            };
            if r.is_err() {
                break;
            }
            count += 1;
        }
        assert_eq!(count, DAILY_TOKENS / RESERVATION);
        assert!(a.reserve(20).is_err());
        assert_eq!(
            b.summary().unwrap()["pending_reserved_tokens"],
            count * RESERVATION
        );
        assert!(b.reserve(86400).is_ok());
        drop(a);
        drop(b);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn a_restart_preserves_usage_and_a_response_can_only_be_recorded_once() {
        let path = path();
        let mut a = Budget::open(&path).unwrap();
        let id = a.reserve(10).unwrap();
        drop(a);
        let b = Budget::open(&path).unwrap();
        b.record(&id, 300, 400).unwrap();
        assert!(b.record(&id, 300, 400).is_err());
        assert!(b.record("missing", 1, 1).is_err());
        let s = b.summary().unwrap();
        assert_eq!(s["input_tokens"], 300);
        assert_eq!(s["output_tokens"], 400);
        assert!(s["estimated_rmb"].is_null());
        assert_eq!(s["remaining_plan_quota_verified"], false);
        drop(b);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn invalid_usage_cannot_release_a_reservation() {
        let path = path();
        let mut a = Budget::open(&path).unwrap();
        let id = a.reserve(10).unwrap();
        assert!(a.record(&id, 18_433, 0).is_err());
        assert!(a.record(&id, 1, 8193).is_err());
        assert_eq!(a.summary().unwrap()["pending_reserved_tokens"], RESERVATION);
        drop(a);
        std::fs::remove_file(path).unwrap();
    }
}
