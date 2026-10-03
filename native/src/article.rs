use super::*;
use buwei_host_core::article::{Article, ArticleStore, Publication};
pub(crate) struct ArticleAdapter {
    pub runtime: Arc<Runtime>,
    pub client: Client,
    pub room: OwnedRoomId,
    pub state: PathBuf,
    pub drop_ack: bool,
}
impl ArticleAdapter {
    fn record(&self, e: &Evidence) -> Result<()> {
        let mut s = ArticleStore::open(&self.state)?;
        let a = s.load()?.ok_or("文章不存在")?;
        s.update(a.revision, |a| a.record(e))?;
        Ok(())
    }
}
impl Adapter for ArticleAdapter {
    fn validate(&self, action: &Action, revision: u64) -> action_receipts::Result<()> {
        let a = ArticleStore::open(&self.state)
            .and_then(|s| s.load())
            .map_err(|_| Error::Integrity)?
            .ok_or(Error::Integrity)?;
        if a.author != account(&self.client)
            || a.room != self.room.as_str()
            || a.action() != *action
            || a.revision != revision
            || a.state != Publication::Draft
        {
            return Err(Error::Conflict);
        }
        Ok(())
    }
    fn dispatch(&self, op: &Operation) -> Dispatch {
        if op.account != account(&self.client) {
            return Dispatch::Rejected("SDK 账号已变化".into());
        }
        if ArticleStore::open(&self.state)
            .and_then(|mut s| s.update(op.revision, |a| a.claim(op)))
            .is_err()
        {
            return Dispatch::Rejected("文章版本或发布内容已变化".into());
        }
        let result = channel::publish(
            &self.runtime,
            &self.client,
            &self.room,
            op,
            "m.room.message",
        );
        if self.drop_ack && result.is_err() {
            eprintln!(
                "{}",
                json!({"fixture_commit_verified":false,"reason":result.as_ref().err()})
            );
        }
        match result {
            Ok(e) if !self.drop_ack => match self.record(&e) {
                Ok(()) => Dispatch::Verified(e),
                Err(_) => Dispatch::Uncertain("文章服务端回执待恢复".into()),
            },
            _ => {
                if let Ok(mut s) = ArticleStore::open(&self.state) {
                    if let Ok(Some(a)) = s.load() {
                        let _ = s.update(a.revision, |a| a.mark_unknown(&op.id));
                    }
                }
                Dispatch::Uncertain("文章沿原编号等待核实".into())
            }
        }
    }
    fn lookup(&self, op: &Operation) -> action_receipts::Result<Option<Evidence>> {
        let found = channel::lookup(
            &self.runtime,
            &self.client,
            &self.room,
            op,
            "m.room.message",
        )?;
        if let Some(e) = &found {
            self.record(e).map_err(|_| Error::Integrity)?;
        }
        Ok(found)
    }
}
pub(crate) fn verify(
    root: &Path,
    rt: Arc<Runtime>,
    client: Client,
    room: OwnedRoomId,
) -> Result<Value> {
    let mut results = vec![];
    for drop_ack in [false, true] {
        // Keep the server's normal rate limits. This fixture sends nine
        // organizer events before the article scene; allow its bucket to refill.
        std::thread::sleep(Duration::from_secs(6));
        let dir = root.join(if drop_ack {
            "article-recovery"
        } else {
            "article-normal"
        });
        std::fs::create_dir_all(&dir).map_err(|_| "文章记录目录不可写")?;
        let authority = Authority::default();
        authority.set_account(Some(&account(&client)));
        let g = authority
            .grant("buwei-article", &["create", "publish"], now(), 3600)
            .map_err(|_| "文章宿主授权失败")?;
        let draft = Article::new(
            account(&client),
            room.to_string(),
            "补位活动小记".into(),
            "# 活动安排\n\n候补按顺序参与，名额以本人接受为准。".into(),
        )?;
        let state = dir.join("article.db");
        ArticleStore::open(&state)?.create(&g, &draft, now())?;
        let adapter = ArticleAdapter {
            runtime: rt.clone(),
            client: client.clone(),
            room: room.clone(),
            state: state.clone(),
            drop_ack,
        };
        let mut j = Journal::open(dir.join("operations.db")).map_err(|_| "文章执行记录不可用")?;
        let op = j
            .prepare(&g, draft.action(), draft.revision, now(), 120)
            .map_err(|_| "文章完整内容预览失败")?;
        j.confirm(&g, &op.id, &adapter, now())
            .map_err(|_| "文章确认失败")?;
        let result = j
            .execute(&g, &op.id, &adapter, now())
            .map_err(|_| "文章执行失败")?;
        if drop_ack {
            if result.status != Status::Unknown {
                return Err("文章不确定状态未保留".into());
            }
            if j.execute(&g, &op.id, &adapter, now()).is_ok() {
                return Err("文章重复执行未拒绝".into());
            }
            drop(j);
            let fresh_authority = Authority::default();
            fresh_authority.set_account(Some(&account(&client)));
            let fresh = fresh_authority
                .grant("buwei-article", &["publish"], now(), 3600)
                .map_err(|_| "文章恢复授权失败")?;
            j = Journal::open(dir.join("operations.db")).map_err(|_| "文章数据库重新打开失败")?;
            let recovered = j
                .reconcile(&fresh, &op.id, &adapter, now())
                .map_err(|_| "文章原编号恢复失败")?;
            if recovered.status != Status::Confirmed || recovered.id != op.id {
                return Err("文章恢复未核实".into());
            }
        } else {
            if result.status != Status::Confirmed {
                return Err("文章缺少服务端回执".into());
            }
            j.execute(&g, &op.id, &adapter, now())
                .map_err(|_| "文章重复确认失败")?;
        }
        let saved = ArticleStore::open(&state)?.load()?.unwrap();
        if saved.state != Publication::Published {
            return Err("文章发布状态未保存".into());
        }
        results.push(json!({"operation_id":op.id,"server_event":saved.server_event,"lost_ack":drop_ack,"published_verified":true,"recovery_resends":0}));
    }
    Ok(
        json!({"scene":"confirmed Markdown message publication","shared_core":"action-receipts","shared_sdk_channel":true,"cases":results,"rinx_original_article_ui_regressed":false}),
    )
}
