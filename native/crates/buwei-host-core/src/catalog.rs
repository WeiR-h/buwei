//! Each activity retains its own state, inbox, synchronization cursor and drafts.
use crate::{Activity, Result, Store};
use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};
pub struct Catalog {
    root: PathBuf,
    db: Connection,
}
impl Catalog {
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_owned();
        std::fs::create_dir_all(root.join("activities")).map_err(|_| "活动目录不可用")?;
        let db = Connection::open(root.join("catalog.db")).map_err(|_| "活动列表不可用")?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS activities(id TEXT PRIMARY KEY,room TEXT UNIQUE NOT NULL);").map_err(|_|"活动列表不可用")?;
        Ok(Self { root, db })
    }
    pub fn path(&self, id: &str) -> Result<PathBuf> {
        if id.len() != 32 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("活动编号不合法".into());
        }
        Ok(self.root.join("activities").join(id).join("activity.db"))
    }
    pub fn list(&self) -> Result<Vec<Activity>> {
        let mut q = self
            .db
            .prepare("SELECT id,room FROM activities ORDER BY rowid DESC")
            .map_err(|_| "活动列表不可用")?;
        let ids = q
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(|_| "活动列表不可用")?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|_| "活动列表不可用")?;
        let mut result = vec![];
        for (id, room) in ids {
            let path = self.path(&id)?;
            if !path.is_file() {
                return Err("活动资料缺失，请保留目录并恢复备份".into());
            }
            let a = Store::open(path)?
                .load()?
                .ok_or("活动资料不完整，请恢复备份")?;
            if a.room != room || a.metadata.as_ref().is_none_or(|m| m.activity_id != id) {
                return Err("活动列表与资料来源不一致".into());
            }
            result.push(a);
        }
        Ok(result)
    }
    pub fn register(&mut self, a: &Activity, clock: u64) -> Result<()> {
        a.validate()?;
        let id = &a
            .metadata
            .as_ref()
            .ok_or("历史活动请从旧版恢复入口查看")?
            .activity_id;
        let count = self
            .list()?
            .iter()
            .filter(|a| a.metadata.as_ref().is_some_and(|m| !m.archived) && a.end > clock)
            .count();
        let exists: bool = self
            .db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM activities WHERE id=?1)",
                [id],
                |r| r.get(0),
            )
            .map_err(|_| "活动列表不可用")?;
        if !exists && count >= 5 {
            return Err("同时举办最多五场活动，请先结束或归档一场活动".into());
        }
        let stored = Store::open(self.path(id)?)?
            .load()?
            .ok_or("活动尚未持久化")?;
        if stored.room != a.room || stored.owner != a.owner {
            return Err("活动来源不一致".into());
        }
        self.db
            .execute(
                "INSERT INTO activities VALUES(?1,?2) ON CONFLICT(id) DO NOTHING",
                params![id, a.room],
            )
            .map_err(|_| "活动列表登记失败")?;
        Ok(())
    }
    pub fn ensure_capacity(&self, clock: u64) -> Result<()> {
        if self
            .list()?
            .iter()
            .filter(|a| a.metadata.as_ref().is_some_and(|m| !m.archived) && a.end > clock)
            .count()
            >= 5
        {
            return Err("同时举办最多五场活动".into());
        }
        Ok(())
    }
}
