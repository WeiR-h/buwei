//! Community announcements locate an activity; they confer no identity or seats.
use crate::{Activity, Result, account_valid, calendar, room_valid};
use action_receipts::Grant;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeSet, path::Path};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Announcement {
    pub protocol: u8,
    pub activity_id: String,
    pub activity_room: String,
    pub organizer: String,
    pub title: String,
    pub template: String,
    pub location: String,
    pub start: u64,
    pub end: u64,
    pub capacity: u8,
    pub free: u8,
    pub revision: u64,
    pub withdrawn: bool,
}
impl Announcement {
    pub fn from_activity(a: &Activity, withdrawn: bool) -> Result<Self> {
        a.validate()?;
        let m = a.metadata.as_ref().ok_or("历史活动没有社群定位信息")?;
        Ok(Self {
            protocol: 1,
            activity_id: m.activity_id.clone(),
            activity_room: a.room.clone(),
            organizer: a.owner.clone(),
            title: a.title.clone(),
            template: m.template.clone(),
            location: m.location.clone(),
            start: a.start,
            end: a.end,
            capacity: a.capacity,
            free: a.free() as u8,
            revision: a.revision,
            withdrawn,
        })
    }
    pub fn validate(&self) -> Result<()> {
        if self.protocol != 1
            || self.activity_id.len() != 32
            || !self.activity_id.bytes().all(|b| b.is_ascii_hexdigit())
            || !account_valid(&self.organizer)
            || !room_valid(&self.activity_room)
            || self.title.trim().is_empty()
            || self.title.chars().count() > 80
            || self.location.chars().count() > 200
            || !matches!(
                self.template.as_str(),
                "badminton" | "boardgame" | "reading" | "custom"
            )
            || !(1..=30).contains(&self.capacity)
            || self.free > self.capacity
            || self.revision == 0
            || !calendar::interval(self.start, self.end)
            || self.end - self.start > 48 * 3600
        {
            return Err("社群活动卡片不合法".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Listing {
    pub community: String,
    pub event_id: String,
    pub server_time: u64,
    pub announcement: Announcement,
}
impl Listing {
    pub fn parse(event: &Value, room: &str, clock: u64) -> Result<Self> {
        if event["type"] != "m.room.message"
            || event.get("room_id").is_some_and(|r| r != room)
            || !room_valid(room)
        {
            return Err("社群事件对象不一致".into());
        }
        let a: Announcement =
            serde_json::from_value(event["content"]["org.buwei.community"].clone())
                .map_err(|_| "社群活动格式不合法")?;
        a.validate()?;
        let time = event["origin_server_ts"]
            .as_u64()
            .ok_or("社群事件时间缺失")?
            / 1000;
        let id = event["event_id"]
            .as_str()
            .filter(|s| s.starts_with('$') && s.len() > 1)
            .ok_or("社群事件编号缺失")?;
        if event["sender"] != a.organizer || time > clock + 5 {
            return Err("卡片发送者或时间不匹配".into());
        }
        Ok(Self {
            community: room.into(),
            event_id: id.into(),
            server_time: time,
            announcement: a,
        })
    }
    pub fn key(&self) -> String {
        format!("{}|{}", self.community, self.announcement.activity_id)
    }
    pub fn can_request(&self, clock: u64) -> bool {
        !self.announcement.withdrawn && clock < self.announcement.start
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filter {
    pub template: String,
    pub date: String,
    pub available_only: bool,
}
impl Filter {
    pub fn matches(&self, l: &Listing, clock: u64) -> bool {
        l.can_request(clock)
            && (self.template.is_empty()
                || self.template == "any"
                || l.announcement.template == self.template)
            && (self.date.is_empty()
                || calendar::display(l.announcement.start).starts_with(&self.date))
            && (!self.available_only || l.announcement.free > 0)
    }
    pub fn validate(&self) -> Result<()> {
        if !matches!(
            self.template.as_str(),
            "" | "any" | "badminton" | "boardgame" | "reading" | "custom"
        ) {
            return Err("筛选活动类型不合法".into());
        }
        if !self.date.is_empty() {
            calendar::parse(&format!("{} 00:00", self.date))?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntryRequest {
    pub event_id: String,
    pub actor: String,
    pub community: String,
    pub announcement_event: String,
    pub activity_id: String,
    pub activity_room: String,
    pub server_time: u64,
}
impl EntryRequest {
    pub fn parse(event: &Value, community: &str, listing: &Listing, clock: u64) -> Result<Self> {
        let c = &event["content"];
        let time = event["origin_server_ts"]
            .as_u64()
            .ok_or("加入申请时间缺失")?
            / 1000;
        let actor = event["sender"]
            .as_str()
            .filter(|s| account_valid(s))
            .ok_or("加入申请身份缺失")?;
        if event["type"] != "org.buwei.entry.request"
            || event.get("room_id").is_some_and(|r| r != community)
            || listing.community != community
            || c["protocol"] != 1
            || c["announcement_event"] != listing.event_id
            || c["activity_id"] != listing.announcement.activity_id
            || c["activity_room"] != listing.announcement.activity_room
            || time > clock + 5
            || time >= listing.announcement.start
            || actor == listing.announcement.organizer
            || listing.announcement.withdrawn
        {
            return Err("加入申请来源、关联或期限不匹配".into());
        }
        Ok(Self {
            event_id: event["event_id"]
                .as_str()
                .filter(|s| s.starts_with('$') && s.len() > 1)
                .ok_or("申请事件编号缺失")?
                .into(),
            actor: actor.into(),
            community: community.into(),
            announcement_event: listing.event_id.clone(),
            activity_id: listing.announcement.activity_id.clone(),
            activity_room: listing.announcement.activity_room.clone(),
            server_time: time,
        })
    }
}
#[derive(Clone, Debug)]
pub struct AccessPolicy {
    pub account: String,
    pub community: String,
    pub activity_id: String,
    pub announcement_event: String,
    pub expires: u64,
    pub used: BTreeSet<String>,
}
impl AccessPolicy {
    pub fn issue(g: &Grant, l: &Listing, clock: u64, authorization_expires: u64) -> Result<Self> {
        g.check("admit_entry", clock)
            .map_err(|_| "社群访问授权已失效")?;
        if g.account() != l.announcement.organizer || !l.can_request(clock) {
            return Err("只能由本场组织者授权加入申请".into());
        }
        Ok(Self {
            account: g.account().into(),
            community: l.community.clone(),
            activity_id: l.announcement.activity_id.clone(),
            announcement_event: l.event_id.clone(),
            expires: authorization_expires
                .min(clock + 3600)
                .min(l.announcement.start),
            used: Default::default(),
        })
    }
    pub fn check(
        &self,
        g: &Grant,
        r: &EntryRequest,
        current_member: bool,
        clock: u64,
    ) -> Result<()> {
        g.check("admit_entry", clock)
            .map_err(|_| "社群访问授权已失效")?;
        if self.account != g.account()
            || clock >= self.expires
            || !current_member
            || self.community != r.community
            || self.activity_id != r.activity_id
            || self.announcement_event != r.announcement_event
            || r.actor == self.account
            || (!self.used.contains(&r.event_id) && self.used.len() >= 30)
        {
            return Err("加入申请不在本人已确认的规则范围内".into());
        }
        Ok(())
    }
}
pub struct DiscoveryStore {
    db: Connection,
    account: String,
}
impl DiscoveryStore {
    pub fn open(path: impl AsRef<Path>, account: &str) -> Result<Self> {
        if !account_valid(account) {
            return Err("社群账号未核验".into());
        }
        let db = Connection::open(path).map_err(|_| "社群资料不可打开")?;
        db.execute_batch("PRAGMA journal_mode=WAL;PRAGMA synchronous=FULL;CREATE TABLE IF NOT EXISTS listings(account TEXT NOT NULL,key TEXT NOT NULL,body TEXT NOT NULL,PRIMARY KEY(account,key));CREATE TABLE IF NOT EXISTS cursors(account TEXT NOT NULL,room TEXT NOT NULL,event TEXT NOT NULL,PRIMARY KEY(account,room));CREATE TABLE IF NOT EXISTS entry_requests(account TEXT NOT NULL,event TEXT NOT NULL,body TEXT NOT NULL,status TEXT NOT NULL,operation TEXT,PRIMARY KEY(account,event));").map_err(|_|"社群资料不可初始化")?;
        Ok(Self {
            db,
            account: account.into(),
        })
    }
    pub fn list(&self) -> Result<Vec<Listing>> {
        let mut q = self
            .db
            .prepare("SELECT body FROM listings WHERE account=?1 ORDER BY rowid DESC LIMIT 200")
            .map_err(|_| "社群列表不可读取")?;
        let values = q
            .query_map([&self.account], |r| r.get::<_, String>(0))
            .map_err(|_| "社群列表不可读取")?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|_| "社群列表不可读取")?;
        values
            .iter()
            .map(|s| serde_json::from_str(s).map_err(|_| "社群列表损坏".into()))
            .collect()
    }
    pub fn get(&self, key: &str) -> Result<Listing> {
        self.list()?
            .into_iter()
            .find(|l| l.key() == key)
            .ok_or("活动卡片已变化，请刷新列表".into())
    }
    pub fn cursor(&self, room: &str) -> Result<Option<String>> {
        self.db
            .query_row(
                "SELECT event FROM cursors WHERE account=?1 AND room=?2",
                params![self.account, room],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| "社群同步进度不可读取".into())
    }
    pub fn apply(&mut self, room: &str, events: &[Value], clock: u64) -> Result<()> {
        let existing = self.list()?;
        let mut listings = existing;
        for e in events.iter().filter(|e| {
            e["type"] == "m.room.message" && e["content"].get("org.buwei.community").is_some()
        }) {
            let l = match Listing::parse(e, room, clock) {
                Ok(l) => l,
                Err(_) => continue,
            };
            if let Some(old) = listings.iter_mut().find(|x| x.key() == l.key()) {
                if old.announcement.organizer != l.announcement.organizer
                    || old.announcement.activity_room != l.announcement.activity_room
                {
                    continue;
                }
                if l.server_time >= old.server_time
                    && l.announcement.revision >= old.announcement.revision
                {
                    *old = l;
                }
            } else {
                listings.push(l);
            }
        }
        let tx = self.db.transaction().map_err(|_| "社群资料不可更新")?;
        for l in &listings {
            tx.execute("INSERT INTO listings VALUES(?1,?2,?3) ON CONFLICT(account,key) DO UPDATE SET body=excluded.body",params![self.account,l.key(),serde_json::to_string(l).map_err(|_|"社群卡片不可保存")?]).map_err(|_|"社群卡片不可保存")?;
        }
        for e in events
            .iter()
            .filter(|e| e["type"] == "org.buwei.entry.request")
        {
            if let Some(l) = listings
                .iter()
                .find(|l| l.community == room && e["content"]["announcement_event"] == l.event_id)
            {
                if let Ok(r) = EntryRequest::parse(e, room, l, clock) {
                    tx.execute("INSERT INTO entry_requests VALUES(?1,?2,?3,'pending',NULL) ON CONFLICT(account,event) DO NOTHING",params![self.account,r.event_id,serde_json::to_string(&r).map_err(|_|"申请不可保存")?]).map_err(|_|"申请不可保存")?;
                }
            }
        }
        if let Some(last) = events.last().and_then(|v| v["event_id"].as_str()) {
            tx.execute("INSERT INTO cursors VALUES(?1,?2,?3) ON CONFLICT(account,room) DO UPDATE SET event=excluded.event",params![self.account,room,last]).map_err(|_|"同步进度不可保存")?;
        }
        tx.commit().map_err(|_| "社群资料不可保存")?;
        Ok(())
    }
    pub fn requests(&self) -> Result<Vec<(EntryRequest, String, Option<String>)>> {
        let mut q=self.db.prepare("SELECT body,status,operation FROM entry_requests WHERE account=?1 AND status IN ('pending','unknown') ORDER BY rowid").map_err(|_|"加入申请不可读取")?;
        let rows = q
            .query_map([&self.account], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(|_| "加入申请不可读取")?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|_| "加入申请不可读取")?;
        rows.into_iter()
            .map(|(s, status, op)| {
                Ok((
                    serde_json::from_str(&s).map_err(|_| "加入申请损坏")?,
                    status,
                    op,
                ))
            })
            .collect()
    }
    pub fn set_request(&self, event: &str, status: &str, operation: Option<&str>) -> Result<()> {
        if !matches!(status, "pending" | "unknown" | "invited" | "rejected") {
            return Err("申请状态不合法".into());
        }
        self.db
            .execute(
                "UPDATE entry_requests SET status=?1,operation=?2 WHERE account=?3 AND event=?4",
                params![status, operation, self.account, event],
            )
            .map_err(|_| "申请状态不可保存")?;
        Ok(())
    }
}
