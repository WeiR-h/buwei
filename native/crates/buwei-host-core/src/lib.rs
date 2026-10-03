//! Authoritative activity rules for a trusted native host. Model replies are
//! suggestions; only a host-issued Grant binds the person making an action.
use action_receipts::{Grant, Operation, Status};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, path::Path};

pub type Result<T> = std::result::Result<T, String>;
pub mod article;
pub mod automation;
pub mod calendar;
pub mod catalog;
pub mod facts;
pub mod model_budget;
pub mod participation;
pub mod preference_draft;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    pub earliest: u64,
    pub latest: u64,
    pub group: u8,
}
impl Preferences {
    pub fn validate(&self) -> Result<()> {
        if !(self.earliest <= 23 && self.latest <= 24 && self.earliest < self.latest
            || calendar::interval(self.earliest, self.latest))
            || !(1..=8).contains(&self.group)
        {
            return Err("时段或人数不合法".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelAdvice {
    pub earliest: u64,
    pub latest: u64,
    pub group: u8,
    pub needs_clarification: bool,
    pub explanation: String,
}
impl ModelAdvice {
    pub fn parse(value: &serde_json::Value) -> Result<Self> {
        let advice: Self =
            serde_json::from_value(value.clone()).map_err(|_| "模型格式不符合约定")?;
        advice.preferences().validate()?;
        if advice.explanation.trim().is_empty()
            || advice.explanation.chars().count() > 180
            || advice.explanation.contains("https://")
            || advice.explanation.contains("http://")
        {
            return Err("模型解释不符合约定".into());
        }
        Ok(advice)
    }
    pub fn preferences(&self) -> Preferences {
        Preferences {
            earliest: self.earliest,
            latest: self.latest,
            group: self.group,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersonStatus {
    Waiting,
    Confirmed,
    Declined,
    Cancelled,
    Expired,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Person {
    pub account: String,
    pub name: String,
    pub joined: u64,
    pub preferences: Preferences,
    pub preferences_confirmed: bool,
    pub status: PersonStatus,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    Pending,
    Unknown,
    Delivered,
    Rejected,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reply {
    Pending,
    Accepted,
    Declined,
    Expired,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Invitation {
    pub operation_id: String,
    pub recipient: String,
    pub room: String,
    pub digest: String,
    pub until: u64,
    pub delivery: Delivery,
    pub server_event: Option<String>,
    pub reply: Reply,
}
impl Invitation {
    fn holds(&self) -> bool {
        self.reply == Reply::Pending && self.delivery != Delivery::Rejected
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Activity {
    pub owner: String,
    pub room: String,
    pub title: String,
    pub capacity: u8,
    pub start: u64,
    pub end: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<calendar::Metadata>,
    pub revision: u64,
    pub serial: u64,
    pub paused: bool,
    pub people: Vec<Person>,
    pub invitations: Vec<Invitation>,
}
fn matrix_id(s: &str, prefix: char) -> bool {
    s.starts_with(prefix)
        && s[1..]
            .split_once(':')
            .is_some_and(|(local, server)| !local.is_empty() && !server.is_empty())
        && s.len() <= 255
        && !s.chars().any(|c| c.is_whitespace() || c.is_control())
}
pub fn account_valid(s: &str) -> bool {
    matrix_id(s, '@')
}
// Modern Matrix rooms have opaque IDs without a server-name suffix. The
// adapter still obtains a typed RoomId from the native SDK and binds it exactly.
fn room_valid(s: &str) -> bool {
    s.starts_with('!')
        && s.len() > 1
        && s.len() <= 255
        && !s.chars().any(|c| c.is_whitespace() || c.is_control())
}
fn event_valid(s: &str) -> bool {
    s.starts_with('$')
        && s.len() > 1
        && s.len() <= 255
        && !s.chars().any(|c| c.is_whitespace() || c.is_control())
}
fn hex_valid(s: &str, len: usize) -> bool {
    s.len() == len && s.bytes().all(|b| b.is_ascii_hexdigit())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InvitePayload {
    person: String,
    until: u64,
}
impl Activity {
    pub fn new(
        owner: String,
        room: String,
        title: String,
        capacity: u8,
        start: u64,
        end: u64,
    ) -> Result<Self> {
        let a = Self {
            owner,
            room,
            title,
            capacity,
            start,
            end,
            metadata: None,
            revision: 1,
            serial: 0,
            paused: false,
            people: vec![],
            invitations: vec![],
        };
        a.validate()?;
        Ok(a)
    }
    pub fn confirmed(&self) -> usize {
        self.people
            .iter()
            .filter(|p| p.status == PersonStatus::Confirmed)
            .map(|p| {
                if self.metadata.is_some() {
                    p.preferences.group as usize
                } else {
                    1
                }
            })
            .sum()
    }
    pub fn held(&self) -> usize {
        self.invitations
            .iter()
            .filter(|i| i.holds())
            .map(|i| {
                if self.metadata.is_some() {
                    self.people
                        .iter()
                        .find(|p| p.account == i.recipient)
                        .map(|p| p.preferences.group as usize)
                        .unwrap_or(0)
                } else {
                    1
                }
            })
            .sum()
    }
    /// Shared ordering for the visible roster and deidentified model summary.
    pub fn ordered_people(&self) -> Vec<&Person> {
        let mut people = self.people.iter().collect::<Vec<_>>();
        people.sort_by_key(|p| p.joined);
        people
    }
    pub fn free(&self) -> usize {
        (self.capacity as usize).saturating_sub(self.confirmed() + self.held())
    }
    pub fn validate(&self) -> Result<()> {
        if self.revision == 0
            || self.revision > i64::MAX as u64
            || self.serial > i64::MAX as u64
            || !account_valid(&self.owner)
            || !room_valid(&self.room)
            || self.title.trim().is_empty()
            || self.title.chars().count() > 80
            || !(1..=30).contains(&self.capacity)
            || self.start >= self.end
            || (if self.metadata.is_some() {
                !calendar::interval(self.start, self.end) || self.end - self.start > 48 * 3600
            } else {
                self.end > 24
            })
        {
            return Err("活动信息不合法".into());
        }
        if self.people.len() > if self.metadata.is_some() { 300 } else { 30 }
            || self.effective_registrations() > 30
            || self.invitations.len() > 1000
            || self.confirmed() + self.held() > self.capacity as usize
        {
            return Err("活动容量或记录数量不合法".into());
        }
        if let Some(meta) = &self.metadata {
            meta.validate()?;
        }
        let mut ids = BTreeSet::new();
        let mut positions = BTreeSet::new();
        for p in &self.people {
            p.preferences.validate()?;
            if self.metadata.is_some() != (p.preferences.earliest >= calendar::MIN_TIME) {
                return Err("报名日期必须与本场活动的时间格式一致".into());
            }
            if !account_valid(&p.account)
                || !ids.insert(&p.account)
                || p.name.trim().is_empty()
                || p.name.chars().count() > 20
                || p.joined == 0
                || p.joined > self.serial
                || !positions.insert(p.joined)
            {
                return Err("参与者信息不合法".into());
            }
        }
        let mut operations = BTreeSet::new();
        let mut active = BTreeSet::new();
        for i in &self.invitations {
            if !operations.insert(&i.operation_id)
                || !hex_valid(&i.operation_id, 32)
                || !hex_valid(&i.digest, 64)
                || i.room != self.room
                || !ids.contains(&i.recipient)
                || i.until == 0
            {
                return Err("邀请信息不合法".into());
            }
            if i.holds()
                && (!active.insert(&i.recipient)
                    || self.person(&i.recipient)?.status != PersonStatus::Waiting)
            {
                return Err("重复或冲突的名额保留".into());
            }
            if i.delivery == Delivery::Delivered
                && i.server_event.as_deref().is_none_or(|s| !event_valid(s))
            {
                return Err("送达状态缺少服务端证据".into());
            }
            if i.reply != Reply::Pending && i.delivery != Delivery::Delivered {
                return Err("回复结果缺少邀请送达证据".into());
            }
        }
        Ok(())
    }
    fn person(&self, account: &str) -> Result<&Person> {
        self.people
            .iter()
            .find(|p| p.account == account)
            .ok_or("参与者不存在".into())
    }
    pub fn effective_registrations(&self) -> usize {
        if self.metadata.is_none() {
            return self.people.len();
        }
        self.people
            .iter()
            .filter(|p| matches!(p.status, PersonStatus::Waiting | PersonStatus::Confirmed))
            .count()
    }
    pub fn add_person(
        &mut self,
        account: String,
        name: String,
        preferences: Preferences,
        confirmed: bool,
    ) -> Result<()> {
        if self.effective_registrations() >= 30
            || self.people.len() >= if self.metadata.is_some() { 300 } else { 30 }
            || self.people.iter().any(|p| p.account == account)
        {
            return Err("重复参与者或人数已达上限".into());
        }
        let mut next = self.clone();
        next.serial += 1;
        next.people.push(Person {
            account,
            name,
            preferences,
            preferences_confirmed: confirmed,
            status: PersonStatus::Waiting,
            joined: next.serial,
        });
        next.revision += 1;
        next.validate()?;
        *self = next;
        Ok(())
    }
    /// The adapter supplies the sender of a server-verified join event. A
    /// waiting person's edits preserve their place; returning after leaving
    /// creates a new queue position and never displaces an existing invite.
    pub fn join_own(
        &mut self,
        actor: String,
        name: String,
        preferences: Preferences,
    ) -> Result<()> {
        preferences.validate()?;
        if let Some(index) = self.people.iter().position(|p| p.account == actor) {
            if self.people[index].status == PersonStatus::Waiting {
                return self.confirm_preferences(&actor, preferences);
            }
            if self.people[index].status == PersonStatus::Confirmed
                || self
                    .invitations
                    .iter()
                    .any(|i| i.recipient == actor && i.holds())
            {
                return Err("先处理本人已确认的席位或当前邀请".into());
            }
            let mut next = self.clone();
            next.serial += 1;
            let p = &mut next.people[index];
            p.joined = next.serial;
            p.preferences = preferences;
            p.preferences_confirmed = true;
            p.status = PersonStatus::Waiting;
            next.revision += 1;
            next.validate()?;
            *self = next;
            Ok(())
        } else {
            self.add_person(actor, name, preferences, true)
        }
    }
    fn eligible(&self, p: &Person) -> bool {
        p.status == PersonStatus::Waiting
            && p.preferences_confirmed
            && (if self.metadata.is_some() {
                p.preferences.group as usize <= self.free()
            } else {
                p.preferences.group == 1
            })
            && p.preferences.earliest <= self.start
            && p.preferences.latest >= self.end
            && !self
                .invitations
                .iter()
                .any(|i| i.recipient == p.account && i.holds())
    }
    pub fn candidate(&self) -> Option<&str> {
        if self.paused || self.metadata.as_ref().is_some_and(|m| m.archived) || self.free() == 0 {
            return None;
        }
        self.people
            .iter()
            .filter(|p| self.eligible(p))
            .min_by_key(|p| p.joined)
            .map(|p| p.account.as_str())
    }
    pub fn cancel_own(&mut self, actor: &str) -> Result<()> {
        if self.metadata.is_some()
            && self
                .invitations
                .iter()
                .any(|i| i.recipient == actor && i.holds())
        {
            return Err("请先接受或拒绝当前邀请，再取消报名".into());
        }
        let p = self
            .people
            .iter_mut()
            .find(|p| p.account == actor)
            .ok_or("当前身份不属于活动")?;
        if p.status != PersonStatus::Confirmed
            && !(self.metadata.is_some() && p.status == PersonStatus::Waiting)
        {
            return Err("只能取消本人的候补报名或已确认席位".into());
        }
        if let Some(m) = self.metadata.as_mut() {
            m.metrics.cancelled_people += p.preferences.group as u32;
        }
        p.status = PersonStatus::Cancelled;
        self.revision += 1;
        Ok(())
    }
    /// Applying model advice alone does nothing. This is called only after the
    /// person's host confirmation of the exact typed preferences.
    pub fn confirm_preferences(&mut self, actor: &str, preferences: Preferences) -> Result<()> {
        preferences.validate()?;
        if self
            .invitations
            .iter()
            .any(|i| i.recipient == actor && i.holds())
        {
            return Err("先处理当前邀请".into());
        }
        let p = self
            .people
            .iter_mut()
            .find(|p| p.account == actor)
            .ok_or("当前身份不属于活动")?;
        p.preferences = preferences;
        p.preferences_confirmed = true;
        self.revision += 1;
        Ok(())
    }
    pub fn reserve(&mut self, operation: &Operation, now: u64) -> Result<()> {
        if operation.account != self.owner
            || operation.app != "buwei"
            || operation.action.permission != "invite"
            || operation.action.target != self.room
        {
            return Err("邀请的身份或活动不一致".into());
        }
        let expected = hex::encode(Sha256::digest(
            serde_json::to_vec(&operation.action).map_err(|_| "动作格式不合法")?,
        ));
        if expected != operation.digest {
            return Err("动作摘要不一致".into());
        }
        let payload: InvitePayload = serde_json::from_value(operation.action.payload.clone())
            .map_err(|_| "邀请参数不符合约定")?;
        if let Some(i) = self
            .invitations
            .iter()
            .find(|i| i.operation_id == operation.id)
        {
            return if i.digest == operation.digest
                && i.recipient == payload.person
                && i.until == payload.until
            {
                Ok(())
            } else {
                Err("原编号内容不一致".into())
            };
        }
        if self.metadata.is_some() && (now >= self.start || payload.until > self.start) {
            return Err("活动已开始或邀请截止超过活动开始时间".into());
        }
        if operation.status != Status::Dispatching || operation.revision != self.revision {
            return Err("邀请尚未确认执行或业务版本已变化".into());
        }
        let person = payload.person.as_str();
        let until = payload.until;
        if self.candidate() != Some(person) || until <= now || until > now.saturating_add(3600) {
            return Err("顺序、时段、名额或有效期已变化".into());
        }
        let mut next = self.clone();
        next.invitations.push(Invitation {
            operation_id: operation.id.clone(),
            recipient: person.into(),
            room: self.room.clone(),
            digest: operation.digest.clone(),
            until,
            delivery: Delivery::Pending,
            server_event: None,
            reply: Reply::Pending,
        });
        next.revision += 1;
        next.validate()?;
        *self = next;
        Ok(())
    }
    pub fn mark_unknown(&mut self, id: &str) -> Result<()> {
        let i = self
            .invitations
            .iter_mut()
            .find(|i| i.operation_id == id)
            .ok_or("邀请不存在")?;
        if i.delivery == Delivery::Pending {
            i.delivery = Delivery::Unknown;
            self.revision += 1;
        }
        Ok(())
    }
    /// The caller must pass sender and room read from the native SDK's event,
    /// never a `sender` field claimed by message content or model output.
    pub fn record_delivery(
        &mut self,
        id: &str,
        sender: &str,
        room: &str,
        event: &str,
        digest: &str,
    ) -> Result<()> {
        if sender != self.owner || room != self.room || !event_valid(event) {
            return Err("邀请送达证据不匹配".into());
        }
        let i = self
            .invitations
            .iter_mut()
            .find(|i| i.operation_id == id)
            .ok_or("邀请不存在")?;
        if i.digest != digest || i.delivery == Delivery::Rejected {
            return Err("邀请内容摘要不匹配".into());
        }
        if let Some(previous) = &i.server_event {
            return if previous == event {
                Ok(())
            } else {
                Err("同一编号出现多个服务端事件".into())
            };
        }
        i.server_event = Some(event.into());
        i.delivery = Delivery::Delivered;
        self.revision += 1;
        Ok(())
    }
    /// Host checks this before sending a reply, including when its snapshot is stale.
    pub fn pending_invitation_for(&self, actor: &str, clock: u64) -> Result<&Invitation> {
        let i = self
            .invitations
            .iter()
            .rev()
            .find(|i| {
                i.recipient == actor
                    && i.reply == Reply::Pending
                    && i.delivery == Delivery::Delivered
            })
            .ok_or("当前账号没有已送达且待回复的邀请")?;
        if clock >= i.until {
            return Err("本人邀请已过期，请刷新活动；不会发送迟到回复".into());
        }
        Ok(i)
    }
    pub fn record_invitation_time(&mut self, id: &str, server_time: u64) -> Result<()> {
        if self.metadata.is_none() {
            return Ok(());
        }
        let _i = self
            .invitations
            .iter()
            .find(|i| i.operation_id == id && i.delivery == Delivery::Delivered)
            .ok_or("邀请送达尚未核验")?;
        if server_time < calendar::MIN_TIME || server_time >= calendar::MAX_TIME {
            return Err("邀请服务端时间不合法".into());
        }
        let metrics = &mut self.metadata.as_mut().unwrap().metrics;
        if let Some(old) = metrics.invitation_times.get(id) {
            return if *old == server_time {
                Ok(())
            } else {
                Err("同一邀请的服务端时间冲突".into())
            };
        }
        metrics.invitation_times.insert(id.into(), server_time);
        self.revision += 1;
        Ok(())
    }
    pub fn receive_reply(
        &mut self,
        id: &str,
        sender: &str,
        room: &str,
        in_reply_to: &str,
        accept: bool,
        server_time: u64,
        now: u64,
    ) -> Result<()> {
        let index = self
            .invitations
            .iter()
            .position(|i| i.operation_id == id)
            .ok_or("邀请不存在")?;
        let i = &self.invitations[index];
        if room != self.room
            || sender != i.recipient
            || i.delivery != Delivery::Delivered
            || i.server_event.as_deref() != Some(in_reply_to)
        {
            return Err("本人身份或回复关联不匹配".into());
        }
        if i.reply != Reply::Pending {
            return Err("邀请已有处理结果".into());
        }
        // A timely server reply remains valid when the organizer reads it late,
        // provided the original reservation is still pending. Expired or
        // reallocated invitations were rejected above. Drain inbox before expiry.
        if server_time >= i.until || server_time > now.saturating_add(5) {
            return Err("迟到或过期回复不能占位".into());
        }
        let mut next = self.clone();
        next.invitations[index].reply = if accept {
            Reply::Accepted
        } else {
            Reply::Declined
        };
        let person = next
            .people
            .iter_mut()
            .find(|p| p.account == sender)
            .ok_or("参与者不存在")?;
        person.status = if accept {
            PersonStatus::Confirmed
        } else {
            PersonStatus::Declined
        };
        if accept {
            if let Some(m) = next.metadata.as_mut() {
                m.metrics.successful_invitations += 1;
                if let Some(sent) = m.metrics.invitation_times.get(id) {
                    if server_time >= *sent {
                        m.metrics.response_seconds += server_time - sent;
                        m.metrics.timed_responses += 1;
                    }
                }
            }
        }
        next.revision += 1;
        next.validate()?;
        *self = next;
        Ok(())
    }
    pub fn expire(&mut self, now: u64) -> usize {
        let mut count = 0;
        for i in &mut self.invitations {
            // An uncertain send remains held even after its deadline, until
            // server evidence or a host-managed decision resolves it.
            if i.reply == Reply::Pending && i.delivery == Delivery::Delivered && i.until <= now {
                i.reply = Reply::Expired;
                if let Some(person) = self.people.iter_mut().find(|p| p.account == i.recipient) {
                    person.status = PersonStatus::Expired;
                }
                count += 1;
            }
        }
        if count > 0 {
            self.revision += 1;
        }
        count
    }
}

pub struct Store {
    db: Connection,
}
impl Store {
    pub fn audit_policy(
        &mut self,
        policy: &str,
        operation: &str,
        digest: &str,
        limit: u8,
    ) -> Result<()> {
        if !hex_valid(policy, 32)
            || !hex_valid(operation, 32)
            || !hex_valid(digest, 64)
            || !(1..=30).contains(&limit)
        {
            return Err("自动规则记录不合法".into());
        }
        self.db.execute_batch("CREATE TABLE IF NOT EXISTS buwei_automation(policy TEXT NOT NULL,operation TEXT PRIMARY KEY,digest TEXT NOT NULL);").map_err(|e|e.to_string())?;
        let tx = self
            .db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let old: Option<(String, String)> = tx
            .query_row(
                "SELECT policy,digest FROM buwei_automation WHERE operation=?1",
                [operation],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some((p, d)) = old {
            return if p == policy && d == digest {
                Ok(())
            } else {
                Err("自动规则编号或摘要冲突".into())
            };
        }
        let count: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM buwei_automation WHERE policy=?1",
                [policy],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if count >= limit as i64 {
            return Err("本次自动补位发送上限已达到，请重新查看规则".into());
        }
        tx.execute(
            "INSERT INTO buwei_automation VALUES(?1,?2,?3)",
            params![policy, operation, digest],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let db = Connection::open(path).map_err(|e| e.to_string())?;
        db.busy_timeout(std::time::Duration::from_secs(3))
            .map_err(|e| e.to_string())?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS buwei_state(singleton INTEGER PRIMARY KEY CHECK(singleton=1), revision INTEGER NOT NULL, body TEXT NOT NULL); CREATE TABLE IF NOT EXISTS buwei_inbox(event_id TEXT PRIMARY KEY, accepted INTEGER NOT NULL); CREATE TABLE IF NOT EXISTS buwei_sync(room TEXT PRIMARY KEY,event_id TEXT NOT NULL);").map_err(|e|e.to_string())?;
        Ok(Self { db })
    }
    pub fn load(&self) -> Result<Option<Activity>> {
        let row: Option<(i64, String)> = self
            .db
            .query_row(
                "SELECT revision,body FROM buwei_state WHERE singleton=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        row.map(|(revision, body)| {
            let state: Activity =
                serde_json::from_str(&body).map_err(|_| "活动记录损坏，原文件保留")?;
            state.validate()?;
            if revision < 1 || state.revision != revision as u64 {
                return Err("活动版本记录不一致".into());
            }
            Ok(state)
        })
        .transpose()
    }
    pub fn sync_checkpoint(&self, room: &str) -> Result<Option<String>> {
        self.db
            .query_row(
                "SELECT event_id FROM buwei_sync WHERE room=?1",
                [room],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())
    }
    /// Save only after a complete SDK history interval was processed. If a
    /// crash happens first, the durable inbox markers deduplicate replay.
    pub fn save_sync_checkpoint(&self, room: &str, event_id: &str) -> Result<()> {
        if !event_valid(event_id) || self.load()?.is_none_or(|a| a.room != room) {
            return Err("同步对象或进度不匹配".into());
        }
        self.db.execute("INSERT INTO buwei_sync VALUES(?1,?2) ON CONFLICT(room) DO UPDATE SET event_id=excluded.event_id",params![room,event_id]).map_err(|e|e.to_string())?;
        Ok(())
    }
    pub fn create(&mut self, actor: &Grant, state: &Activity, now: u64) -> Result<()> {
        actor
            .check("create", now)
            .map_err(|_| "活动创建授权已失效")?;
        state.validate()?;
        if actor.app() != "buwei" || actor.account() != state.owner {
            return Err("活动创建身份不一致".into());
        }
        self.db
            .execute(
                "INSERT INTO buwei_state VALUES(1,?1,?2)",
                params![
                    state.revision as i64,
                    serde_json::to_string(state).map_err(|e| e.to_string())?
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn update(
        &mut self,
        expected: u64,
        change: impl FnOnce(&mut Activity) -> Result<()>,
    ) -> Result<Activity> {
        let tx = self
            .db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let (revision, body): (i64, String) = tx
            .query_row(
                "SELECT revision,body FROM buwei_state WHERE singleton=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|e| e.to_string())?;
        if revision < 1 || revision as u64 != expected {
            return Err("活动版本已变化".into());
        }
        let mut state: Activity =
            serde_json::from_str(&body).map_err(|_| "活动记录损坏，原文件保留")?;
        state.validate()?;
        if state.revision != revision as u64 {
            return Err("活动版本记录不一致".into());
        }
        change(&mut state)?;
        state.validate()?;
        tx.execute(
            "UPDATE buwei_state SET revision=?1,body=?2 WHERE singleton=1",
            params![
                state.revision as i64,
                serde_json::to_string(&state).map_err(|e| e.to_string())?
            ],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(state)
    }
    /// Host adapters call this only for SDK-read events. The effect and replay
    /// marker commit together; a rejected event cannot partly change a queue.
    pub fn apply_verified_event(
        &mut self,
        event_id: &str,
        change: impl FnOnce(&mut Activity) -> Result<()>,
    ) -> Result<Option<bool>> {
        if !event_valid(event_id) {
            return Err("服务端事件编号不合法".into());
        }
        let tx = self
            .db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let exists: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM buwei_inbox WHERE event_id=?1)",
                [event_id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if exists {
            return Ok(None);
        }
        let (revision, body): (i64, String) = tx
            .query_row(
                "SELECT revision,body FROM buwei_state WHERE singleton=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|e| e.to_string())?;
        let mut state: Activity = serde_json::from_str(&body).map_err(|_| "活动记录损坏")?;
        state.validate()?;
        if revision < 1 || state.revision != revision as u64 {
            return Err("活动版本记录不一致".into());
        }
        let accepted = change(&mut state).and_then(|_| state.validate()).is_ok();
        if accepted {
            tx.execute(
                "UPDATE buwei_state SET revision=?1,body=?2 WHERE singleton=1",
                params![
                    state.revision as i64,
                    serde_json::to_string(&state).map_err(|e| e.to_string())?
                ],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.execute(
            "INSERT INTO buwei_inbox VALUES(?1,?2)",
            params![event_id, accepted as i64],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(Some(accepted))
    }
    /// Read-only participant projection; adapters must first verify the room
    /// creator, state-event sender and exact room through the native SDK.
    pub fn cache_verified_snapshot(&mut self, state: &Activity) -> Result<()> {
        state.validate()?;
        let tx = self
            .db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let old: Option<String> = tx
            .query_row("SELECT body FROM buwei_state WHERE singleton=1", [], |r| {
                r.get(0)
            })
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some(body) = old {
            let previous: Activity = serde_json::from_str(&body).map_err(|_| "缓存损坏")?;
            previous.validate()?;
            if previous.owner != state.owner
                || previous.room != state.room
                || previous.revision > state.revision
            {
                return Err("活动身份或版本回退".into());
            }
            if previous.revision == state.revision
                && serde_json::to_value(&previous).map_err(|e| e.to_string())?
                    != serde_json::to_value(state).map_err(|e| e.to_string())?
            {
                return Err("同版本活动内容不一致".into());
            }
        }
        tx.execute("INSERT INTO buwei_state VALUES(1,?1,?2) ON CONFLICT(singleton) DO UPDATE SET revision=excluded.revision,body=excluded.body",params![state.revision as i64,serde_json::to_string(state).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    }
}
/// Bind a model job to exactly the requirement and state it was requested for.
pub fn advice_digest(text: &str, revision: u64) -> String {
    hex::encode(Sha256::digest(format!("{revision}\0{text}").as_bytes()))
}
