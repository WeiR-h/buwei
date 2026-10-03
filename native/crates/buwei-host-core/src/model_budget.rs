//! One durable spending ceiling for all profiles sharing a provider account.
//! Amounts are conservative estimates in micro RMB, independent of the invoice.
use crate::Result;
use rusqlite::{Connection, params};
use std::path::Path;
pub struct Budget {
    db: Connection,
}
impl Budget {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let db = Connection::open(path).map_err(|_| "共享模型预算不可打开")?;
        db.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|_| "共享预算正在使用")?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS limits(id INTEGER PRIMARY KEY CHECK(id=1),ceiling INTEGER NOT NULL); INSERT OR IGNORE INTO limits VALUES(1,10000000); CREATE TABLE IF NOT EXISTS calls(id TEXT PRIMARY KEY,reserved INTEGER NOT NULL,estimate INTEGER,input INTEGER,output INTEGER); CREATE TABLE IF NOT EXISTS history(source TEXT PRIMARY KEY,estimate INTEGER NOT NULL);").map_err(|_|"共享模型预算不可保存")?;
        Ok(Self { db })
    }
    pub fn configure(&mut self, ceiling_micro_rmb: u64) -> Result<()> {
        if ceiling_micro_rmb == 0 || ceiling_micro_rmb > 1000_000_000 {
            return Err("请设置有效的共享调用预算".into());
        }
        let tx = self
            .db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|_| "共享预算正在使用")?;
        let used: i64 = tx
            .query_row(
                "SELECT COALESCE(SUM(COALESCE(estimate,reserved)),0) FROM calls",
                [],
                |r| r.get(0),
            )
            .map_err(|_| "预算记录不可读取")?;
        if ceiling_micro_rmb < used as u64 {
            return Err("新预算低于已用预估额度".into());
        }
        tx.execute("UPDATE limits SET ceiling=?1", [ceiling_micro_rmb as i64])
            .map_err(|_| "预算不可修改")?;
        tx.commit().map_err(|_| "预算不可保存".into())
    }
    pub fn import_history(&self, source: &str, estimate_micro_rmb: u64) -> Result<()> {
        if source.is_empty() || source.len() > 128 || estimate_micro_rmb > 1000_000_000 {
            return Err("历史预算格式不合法".into());
        }
        self.db.execute("INSERT INTO history VALUES(?1,?2) ON CONFLICT(source) DO UPDATE SET estimate=MAX(estimate,excluded.estimate)",params![source,estimate_micro_rmb as i64]).map_err(|_|"历史预算不可保存")?;
        Ok(())
    }
    pub fn reserve(&mut self) -> Result<String> {
        let tx = self
            .db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|_| "共享预算正在使用")?;
        let ceiling: i64 = tx
            .query_row("SELECT ceiling FROM limits WHERE id=1", [], |r| r.get(0))
            .map_err(|_| "预算上限不可读取")?;
        let used: i64 = tx
            .query_row(
                "SELECT COALESCE(SUM(COALESCE(estimate,reserved)),0) FROM calls",
                [],
                |r| r.get(0),
            )
            .map_err(|_| "预算记录不可读取")?;
        if used + 100000 > ceiling {
            return Err("共享模型预算已达上限，手动功能仍可使用".into());
        }
        let id = action_receipts::new_id();
        tx.execute("INSERT INTO calls(id,reserved) VALUES(?1,100000)", [&id])
            .map_err(|_| "模型调用预留不可保存")?;
        tx.commit().map_err(|_| "模型调用预留不可保存")?;
        Ok(id)
    }
    pub fn record(&self, id: &str, input: u64, output: u64) -> Result<()> {
        if input > 100000 || output > 4096 {
            return Err("模型回传用量超出约定，保留本次预留额度".into());
        }
        let estimate = ((input as f64 * 2.10 + output as f64 * 8.40).ceil()) as i64;
        let n = self
            .db
            .execute(
                "UPDATE calls SET estimate=?2,input=?3,output=?4 WHERE id=?1 AND estimate IS NULL",
                params![id, estimate, input as i64, output as i64],
            )
            .map_err(|_| "模型用量不可保存")?;
        if n != 1 {
            return Err("模型调用编号已记录或不存在".into());
        }
        Ok(())
    }
    pub fn summary(&self) -> Result<serde_json::Value> {
        let(used,input,output,attempts):(i64,i64,i64,i64)=self.db.query_row("SELECT COALESCE(SUM(COALESCE(estimate,reserved)),0),COALESCE(SUM(input),0),COALESCE(SUM(output),0),COUNT(*) FROM calls",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).map_err(|_|"共享模型用量不可读取")?;
        let ceiling: i64 = self
            .db
            .query_row("SELECT ceiling FROM limits WHERE id=1", [], |r| r.get(0))
            .map_err(|_| "预算上限不可读取")?;
        let history: i64 = self
            .db
            .query_row("SELECT COALESCE(SUM(estimate),0) FROM history", [], |r| {
                r.get(0)
            })
            .map_err(|_| "历史模型用量不可读取")?;
        Ok(
            serde_json::json!({"ceiling_rmb":ceiling as f64/1e6,"estimated_or_reserved_rmb":used as f64/1e6,"history_estimated_rmb":history as f64/1e6,"attempts":attempts,"input_tokens":input,"output_tokens":output,"billing_verified":false}),
        )
    }
}
