//! Native SDK adapter for durable participant actions.
use super::*;
use buwei_host_core::participation::Intent;
pub(crate) struct ParticipantAdapter {
    pub runtime: Arc<Runtime>,
    pub client: Client,
    pub room: OwnedRoomId,
    pub state: PathBuf,
    pub drop_ack: bool,
}
impl ParticipantAdapter {
    fn intent(&self, op: &Operation) -> action_receipts::Result<Intent> {
        Intent::parse(&op.action).map_err(|_| Error::Integrity)
    }
    fn record(&self, op: &Operation, e: &Evidence) -> Result<()> {
        #[cfg(feature = "full-host")]
        if rinx_bridge::official_mode() {
            return Ok(());
        }
        let intent = Intent::parse(&op.action)?;
        let v = event(
            &self.runtime,
            &self.client,
            &self.room,
            OwnedEventId::try_from(e.external_id.as_str()).map_err(|_| "事件编号不合法")?,
        )?;
        let t = v["origin_server_ts"].as_u64().ok_or("服务端时间缺失")? / 1000;
        Store::open(&self.state)?.apply_verified_event(&e.external_id, |a| match intent {
            Intent::Join { preferences } => {
                a.join_own(e.account.clone(), "参与者".into(), preferences)
            }
            Intent::Reply {
                invitation_id,
                invitation_event,
                accept,
            } => a.receive_reply(
                &invitation_id,
                &e.account,
                &e.target,
                &invitation_event,
                accept,
                t,
                now(),
            ),
            Intent::Cancel => a.cancel_own(&e.account),
        })?;
        Ok(())
    }
}
impl Adapter for ParticipantAdapter {
    fn validate(&self, action: &Action, revision: u64) -> action_receipts::Result<()> {
        let a = Store::open(&self.state)
            .and_then(|s| s.load())
            .map_err(|_| Error::Integrity)?
            .ok_or(Error::Integrity)?;
        if a.room != self.room.as_str() || action.target != a.room || a.revision != revision {
            return Err(Error::Conflict);
        }
        Intent::parse(action)
            .and_then(|i| i.validate(&a, &account(&self.client), now()))
            .map_err(|_| Error::Conflict)
    }
    fn dispatch(&self, op: &Operation) -> Dispatch {
        if account(&self.client) != op.account {
            return Dispatch::Rejected("SDK 账号已变化".into());
        }
        let intent = match self.intent(op) {
            Ok(i) => i,
            Err(_) => return Dispatch::Rejected("参与者操作格式不符合约定".into()),
        };
        match channel::publish(
            &self.runtime,
            &self.client,
            &self.room,
            op,
            intent.event_type(),
        ) {
            Ok(e) if !self.drop_ack => match self.record(op, &e) {
                Ok(()) => Dispatch::Verified(e),
                Err(_) => Dispatch::Uncertain("回执已送达，业务记录等待恢复".into()),
            },
            _ => Dispatch::Uncertain("本人操作结果待核实；原编号保留，禁止重新发送".into()),
        }
    }
    fn lookup(&self, op: &Operation) -> action_receipts::Result<Option<Evidence>> {
        let intent = self.intent(op)?;
        let found = channel::lookup(
            &self.runtime,
            &self.client,
            &self.room,
            op,
            intent.event_type(),
        )?;
        if let Some(e) = &found {
            self.record(op, e).map_err(|_| Error::Integrity)?;
        }
        Ok(found)
    }
}
