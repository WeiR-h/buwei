//! Serialized native host. UI commands contain neither credentials nor actors.
use super::*;
use buwei_host_core::article::{Article, ArticleStore};
use buwei_host_core::{
    advice_digest,
    preference_draft::{PreferenceDraft, dialogue_binding},
};
#[derive(Clone)]
#[cfg_attr(feature = "acceptance", derive(serde::Deserialize))]
#[cfg_attr(
    feature = "acceptance",
    serde(tag = "command", content = "value", deny_unknown_fields)
)]
pub(crate) enum Command {
    Assistance(intentions::IntentCommand),
    CreateDated(community::ActivityForm),
    UseTemplate(String),
    SelectActivity(usize),
    CopyActivity,
    ArchiveActivity,
    RecoverSetup,
    LoadContacts,
    PrepareShare(String),
    ConfirmShare(String),
    OpenCard {
        room: String,
        activity_id: String,
    },
    JoinCard,
    PreviewAutomation(buwei_host_core::automation::Settings),
    ConfirmAutomation {
        id: String,
        settings: buwei_host_core::automation::Settings,
    },
    PauseAutomation,
    PauseAutomationFor(String),
    CreateWithAi(String),
    ApplyActivityDraft,
    Ask(String),
    SuggestDated {
        requirement: String,
        answer: String,
    },
    ApplyDatedSuggestion {
        requirement: String,
        answer: String,
    },
    Authorize,
    ConfirmAuthorization(String),
    Switch,
    Revoke,
    JoinRoom(String),
    InviteMember(String),
    SyncActivity,
    Create {
        title: String,
        capacity: u8,
        start: u8,
        end: u8,
    },
    Join(Preferences),
    ConfirmParticipant(Preferences),
    ReconcilePending,
    Prepare,
    Execute,
    Reconcile,
    Accept(bool),
    Cancel,
    Expire,
    Suggest(String),
    Clarify {
        requirement: String,
        answer: String,
    },
    ApplySuggestion(String),
    Explain,
    GenerateNote,
    ConfigureModel,
    ApplyNote {
        title: String,
        markdown: String,
    },
    Draft {
        title: String,
        markdown: String,
    },
    NewArticle {
        title: String,
        markdown: String,
    },
    PrepareArticle,
    PublishArticle {
        title: String,
        markdown: String,
    },
    ReconcileArticle,
    Fault,
    Refresh,
    ShowTodos,
    #[cfg(feature = "acceptance")]
    TestFault(String),
    #[cfg(feature = "acceptance")]
    TestTrayAction(String),
    #[cfg(feature = "acceptance")]
    TestContact(String),
    #[cfg(feature = "acceptance")]
    TestJoinContact(String),
    #[cfg(feature = "acceptance")]
    CollectEvidence,
    #[cfg(feature = "acceptance")]
    CollectEvidenceSince(String),
    #[cfg(feature = "acceptance")]
    TestLegacyArticle {
        title: String,
        markdown: String,
    },
}
#[derive(Clone, Default)]
pub(crate) struct View {
    pub goals: Vec<(String, String)>,
    pub goal_form: Option<intentions::GoalForm>,
    pub goal_sources: String,
    pub goal_id: Option<String>,
    pub personal_preferences: Option<buwei_host_core::assistance::PersonalPreferences>,
    pub intention_result: String,
    pub assistance_cards: Vec<buwei_host_core::proactive::AssistanceCard>,
    pub assistance_route: Option<u8>,
    pub assistance_tasks: String,
    pub task_choices: Vec<(String, String)>,
    pub latest_feedback: Option<(String, String)>,
    pub background_status: String,
    pub analysis_consent_id: Option<String>,
    pub analysis_status: String,
    pub activity_identity: String,
    pub dated: bool,
    pub activity_list: Vec<String>,
    pub create_form: Option<community::ActivityForm>,
    pub contacts: Vec<String>,
    pub card_status: String,
    pub share_preview: String,
    pub automation_status: String,
    pub policy_consent_id: Option<String>,
    pub policy_preview: String,
    pub dated_inputs: Option<(String, String, u8)>,
    pub ai_result: String,
    pub confirmation_ready: bool,
    pub success: bool,
    pub account: String,
    pub activity: String,
    pub people: String,
    pub preview: String,
    pub participant_preview: String,
    pub participant_inputs: Option<(String, Preferences)>,
    pub history: String,
    pub reply: String,
    pub article: String,
    pub draft: Option<(String, String)>,
    pub generated_note: Option<(String, String, String)>,
    pub model_status: String,
    pub advice: String,
    pub message: String,
    pub sync_status: String,
    pub organizer: bool,
    pub authorized: bool,
    pub consent_id: Option<String>,
    pub consent: String,
    pub expires: String,
    pub fault: bool,
}
pub(crate) struct Controller {
    goal_form: Option<intentions::GoalForm>,
    goal_ai_draft: bool,
    selected_goal: Option<String>,
    intention_result: String,
    assistance_route: Option<u8>,
    active_task: Option<String>,
    expiration_notified: u64,
    analysis_consent: Option<consent::Consent>,
    analysis_until: u64,
    last_analysis: u64,
    dated_dialogue: Option<(String, Vec<String>)>,
    selected_activity: Option<String>,
    sync_rotation: usize,
    sync_results: std::collections::BTreeMap<String, String>,
    policies: std::collections::HashMap<String, buwei_host_core::automation::Policy>,
    pub(super) automation_interlock: automation_guard::Interlock,
    policy_permits: std::collections::HashMap<String, automation_guard::Permit>,
    policy_preview: Option<(
        String,
        String,
        buwei_host_core::automation::Settings,
        u64,
        automation_guard::Permit,
    )>,
    contacts: Vec<(String, String)>,
    share_current: Option<Operation>,
    incoming_card: Option<(String, String)>,
    create_form: Option<community::ActivityForm>,
    dated_suggestion: Option<(String, String, u64, community::DatedDraft)>,
    ai_result: String,
    root: PathBuf,
    consent: Option<consent::Consent>,
    authorized_until: u64,
    server_verified: bool,
    data: PathBuf,
    rt: Arc<Runtime>,
    owner: Client,
    participant: Client,
    selected: bool,
    authority: Authority,
    grant: Option<Grant>,
    article_grant: Option<Grant>,
    sync_grant: Option<Grant>,
    last_sync_status: String,
    journal: Journal,
    current: Option<Operation>,
    participant_current: Option<Operation>,
    article_current: Option<Operation>,
    fault: bool,
    model: Arc<octosense_llm_service::complete::ModelHost>,
    suggestion: Option<(String, String, u64, PreferenceDraft, Vec<String>)>,
    note: Option<(String, u64, model::Note)>,
    explanation: Option<String>,
    message: String,
}
impl Controller {
    pub fn open(root: &Path) -> Result<Self> {
        #[cfg(feature = "full-host")]
        if rinx_bridge::official_mode() {
            return Self::open_official(root);
        }
        let sessions = private_sessions(&root.join(".run/matrix/identities.dpapi"))?;
        let rt = Arc::new(Runtime::new().map_err(|_| "SDK 运行环境不可用")?);
        let owner = client(
            &rt,
            sessions.get("organizer").ok_or("组织者配置缺失")?.clone(),
        )?;
        let participant = client(
            &rt,
            sessions.get("participant").ok_or("参与者配置缺失")?.clone(),
        )?;
        if account(&owner) == account(&participant) {
            return Err("需要两个不同的真实测试身份".into());
        }
        let data = root
            .join("data")
            .join(format!("v{}", env!("CARGO_PKG_VERSION")))
            .join("native");
        std::fs::create_dir_all(&data).map_err(|_| "业务记录目录不可用")?;
        let authority = Authority::default();
        authority.set_account(Some(&account(&owner)));
        let journal = Journal::open(data.join("operations.db")).map_err(|_| "回执数据库不可用")?;
        let selected_activity = community::last_selected(&data);
        Ok(Self {
            goal_form: None,
            goal_ai_draft: false,
            selected_goal: None,
            intention_result: String::new(),
            assistance_route: None,
            active_task: None,
            expiration_notified: 0,
            analysis_consent: None,
            analysis_until: 0,
            last_analysis: 0,
            dated_dialogue: None,
            selected_activity,
            sync_rotation: 0,
            sync_results: Default::default(),
            policies: Default::default(),
            automation_interlock: Default::default(),
            policy_permits: Default::default(),
            policy_preview: None,
            contacts: vec![],
            share_current: None,
            incoming_card: None,
            create_form: None,
            dated_suggestion: None,
            ai_result: String::new(),
            root: root.to_path_buf(),
            consent: None,
            authorized_until: 0,
            server_verified: true,
            data,
            rt,
            owner,
            participant,
            selected: false,
            authority,
            grant: None,
            article_grant: None,
            sync_grant: None,
            last_sync_status: "等待同步授权".into(),
            journal,
            current: None,
            participant_current: None,
            article_current: None,
            fault: false,
            model: model::host(root),
            suggestion: None,
            note: None,
            explanation: None,
            message: "两个本机测试身份已向服务端核验。请先授权；每次执行仍需确认确切预览。".into(),
        })
    }
    #[cfg(feature = "full-host")]
    fn open_official(root: &Path) -> Result<Self> {
        let rt = Arc::new(Runtime::new().map_err(|_| "SDK 运行环境不可用")?);
        let client = rinx_bridge::verified_current(&rt)?;
        let actor = account(&client);
        use sha2::{Digest, Sha256};
        let data = root
            .join("data")
            .join(format!("v{}", env!("CARGO_PKG_VERSION")))
            .join("native/rinx")
            .join(hex::encode(Sha256::digest(actor.as_bytes())));
        std::fs::create_dir_all(&data).map_err(|_| "正式账号记录目录不可用")?;
        let authority = Authority::default();
        authority.set_account(Some(&actor));
        let journal = Journal::open(data.join("operations.db")).map_err(|_| "回执数据库不可用")?;
        rinx_bridge::record_status(root, Some(&actor), true, false)?;
        let selected_activity = community::last_selected(&data);
        Ok(Self {
            goal_form: None,
            goal_ai_draft: false,
            selected_goal: None,
            intention_result: String::new(),
            assistance_route: None,
            active_task: None,
            expiration_notified: 0,
            analysis_consent: None,
            analysis_until: 0,
            last_analysis: 0,
            dated_dialogue: None,
            selected_activity,
            sync_rotation: 0,
            sync_results: Default::default(),
            policies: Default::default(),
            automation_interlock: Default::default(),
            policy_permits: Default::default(),
            policy_preview: None,
            contacts: vec![],
            share_current: None,
            incoming_card: None,
            create_form: None,
            dated_suggestion: None,
            ai_result: String::new(),
            root: root.to_path_buf(),
            consent: None,
            authorized_until: 0,
            server_verified: true,
            data,
            rt,
            owner: client.clone(),
            participant: client,
            selected: false,
            authority,
            grant: None,
            article_grant: None,
            sync_grant: None,
            last_sync_status: "等待同步授权".into(),
            journal,
            current: None,
            participant_current: None,
            article_current: None,
            fault: false,
            model: model::host(root),
            suggestion: None,
            note: None,
            explanation: None,
            message:
                "Rinx 当前真实账号已向服务端核验。请查看授权范围；打开期间每 10 秒同步已核验状态。"
                    .into(),
        })
    }
    pub fn host_session_current(&self) -> bool {
        #[cfg(feature = "full-host")]
        if rinx_bridge::official_mode() {
            return rinx_bridge::ensure_current(self.active()).is_ok();
        }
        true
    }
    fn active(&self) -> &Client {
        if self.selected {
            &self.participant
        } else {
            &self.owner
        }
    }
    fn model_receipt(&self, reply: &Value, revision: u64) -> Result<()> {
        let proof = json!({"version":env!("CARGO_PKG_VERSION"),"returned_model":"MiniMax-M3","official_model_host":true,"schema_validated":true,"usage":reply["meta"]["usage"],"budget":reply["meta"]["budget"],"model_did_not_change_activity":self.activity()?.revision==revision,"billing_verified":false});
        std::fs::write(
            self.data.join("model-receipt.json"),
            serde_json::to_vec_pretty(&proof).map_err(|_| "模型回执格式不合法")?,
        )
        .map_err(|_| "模型结果已生成，但本机回执保存失败".into())
    }
    fn actor(&self) -> String {
        account(self.active())
    }
    fn g(&self, permission: &str) -> Result<Grant> {
        let g = self.grant.as_ref().ok_or("请先授权当前账号")?;
        g.check(permission, now())
            .map_err(|_| "授权失效，请重新授权并预览")?;
        if g.account() != self.actor() {
            return Err("当前 SDK 账号已变化".into());
        }
        Ok(g.clone())
    }
    fn ag(&self) -> Result<Grant> {
        let g = self.article_grant.as_ref().ok_or("请先授权当前账号")?;
        g.check("publish", now()).map_err(|_| "文章授权已失效")?;
        if g.account() != self.actor() {
            return Err("当前 SDK 账号已变化".into());
        }
        Ok(g.clone())
    }
    fn activity(&self) -> Result<Activity> {
        Store::open(self.state_path())?
            .load()?
            .ok_or("请先由组织者创建活动".into())
    }
    fn adapter(&self) -> Result<SdkAdapter> {
        let a = self.activity()?;
        let room = OwnedRoomId::try_from(a.room.as_str()).map_err(|_| "活动房间记录不合法")?;
        Ok(SdkAdapter {
            runtime: self.rt.clone(),
            client: self.owner.clone(),
            room,
            state: self.state_path(),
            drop_ack: self.fault,
            automation_permit: None,
        })
    }
    fn article_path(&self) -> PathBuf {
        use sha2::{Digest, Sha256};
        let actor = hex::encode(Sha256::digest(self.actor().as_bytes()));
        self.data.join(match &self.selected_activity {
            Some(id) => format!("article-{id}-{actor}.db"),
            None => format!("article-{actor}.db"),
        })
    }
    fn article_adapter(&self) -> Result<article::ArticleAdapter> {
        let a = self.activity()?;
        Ok(article::ArticleAdapter {
            runtime: self.rt.clone(),
            client: self.active().clone(),
            room: OwnedRoomId::try_from(a.room.as_str()).map_err(|_| "房间记录不合法")?,
            state: self.article_path(),
            drop_ack: self.fault,
        })
    }
    pub fn handle(&mut self, command: Command) -> View {
        self.assistance_route = None;
        let intention_command = matches!(&command, Command::Assistance(_));
        let outcome = self.apply(command);
        let success = outcome.is_ok();
        self.message = match outcome {
            Ok(m) => m,
            Err(m) => m,
        };
        if intention_command {
            self.intention_result = self.message.clone();
        }
        #[cfg(feature = "full-host")]
        if rinx_bridge::official_mode() {
            let _ = rinx_bridge::record_status(
                &self.root,
                Some(&self.actor()),
                self.server_verified && self.host_session_current(),
                self.grant
                    .as_ref()
                    .is_some_and(|g| g.check("read", now()).is_ok()),
            );
        }
        let _ = self.refresh_assistance();
        let mut view = self.view();
        view.success = success;
        view
    }
    fn apply(&mut self, command: Command) -> Result<String> {
        #[cfg(feature = "acceptance")]
        if let Command::TestTrayAction(name) = &command {
            crate::tray::test_action(name)?;
            return Ok("已调用与托盘菜单相同的宿主动作；记录窗口状态及后续执行结果。".into());
        }
        // Revocation and pause must work even if identity preflight is offline.
        if matches!(&command, Command::Revoke) {
            self.analysis_until = 0;
            self.analysis_consent = None;
            self.automation_interlock.invalidate_all();
            self.policies.clear();
            self.policy_permits.clear();
            self.policy_preview = None;
            self.dated_suggestion = None;
            self.dated_dialogue = None;
            for g in [&self.grant, &self.article_grant, &self.sync_grant]
                .into_iter()
                .flatten()
            {
                g.revoke();
            }
            self.authorized_until = 0;
            self.consent = None;
            self.suggestion = None;
            self.note = None;
            self.explanation = None;
            self.last_sync_status = "授权已撤销，自动同步停止".into();
            return Ok("当前账号的授权已撤销。".into());
        }
        if let Some(id) = match &command {
            Command::PauseAutomation => self.selected_activity.clone(),
            Command::PauseAutomationFor(id) => Some(id.clone()),
            _ => None,
        } {
            self.automation_interlock.invalidate(&id);
            self.policies.remove(&id);
            self.policy_permits.remove(&id);
            self.policy_preview = None;
            return Ok("自动补位已暂停。".into());
        }
        #[cfg(feature = "full-host")]
        if rinx_bridge::official_mode() {
            self.server_verified = false;
            if let Err(e) = rinx_bridge::verify_current(&self.rt, self.active()) {
                if !self.host_session_current() {
                    self.shutdown();
                }
                return Err(e);
            }
            self.server_verified = true;
            match &command {
                Command::Switch => {
                    return Err("请在 Rinx 中退出或切换账号；补位不能自行声明其他身份。".into());
                }
                Command::Fault => return Err("正式服务关闭故障注入；请在本机测试环境验收。".into()),
                _ => {}
            }
        }
        if community::is_community_command(&command) {
            return self.apply_community(command);
        }
        if let Command::Assistance(command) = command {
            return self.apply_intention(command);
        }
        match command {
            #[cfg(feature = "acceptance")]
            Command::TestJoinContact(room) => {
                self.g("read")?;
                let room =
                    OwnedRoomId::try_from(room.as_str()).map_err(|_| "测试联系人房间不合法")?;
                self.rt
                    .block_on(self.active().join_room_by_id(&room))
                    .map_err(|_| "测试联系人房间尚未加入")?;
                self.load_contacts()?;
                Ok("已加入独立测试会话".into())
            }
            #[cfg(feature = "acceptance")]
            Command::TestContact(recipient) => {
                self.g("read")?;
                if !std::env::args().any(|a| a == "--acceptance") {
                    return Err("验收控制未启用".into());
                }
                let user = matrix_sdk::ruma::OwnedUserId::try_from(recipient.as_str())
                    .map_err(|_| "测试联系人格式不合法")?;
                if user.as_str() == self.actor() {
                    return Err("测试联系人须为另一身份".into());
                }
                let room = match self.active().get_dm_room(&user) {
                    Some(r) => r,
                    None => self
                        .rt
                        .block_on(self.active().create_dm(&user))
                        .map_err(|_| "测试联系人创建结果待核实，请先在 Rinx 查看原房间")?,
                };
                self.load_contacts()?;
                Ok(format!("已连接独立测试联系人：{}", room.room_id()))
            }
            Command::ConfigureModel => {
                let exe = std::env::current_exe().map_err(|_| "配置工具位置不可读取")?;
                let script = exe
                    .ancestors()
                    .filter_map(|p| {
                        let path = p.join("tools/Configure-Model.ps1");
                        path.is_file().then_some(path)
                    })
                    .next()
                    .ok_or("配置工具未随程序安装，请使用完整运行包")?;
                std::process::Command::new("powershell.exe")
                    .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
                    .arg(script)
                    .arg("-ProfileDirectory")
                    .arg(&self.root)
                    .spawn()
                    .map_err(|_| "配置窗口无法启动")?;
                Ok("请在独立的本机配置窗口填写密钥。密钥不进入聊天、动作参数或公开材料。".into())
            }
            #[cfg(feature = "acceptance")]
            Command::TestFault(mode) => {
                self.g("read")?;
                super::acceptance::set_fault(&mode)?;
                Ok("仅验收构建的单次传输故障已设置；公开运行包不包含此功能。".into())
            }
            #[cfg(feature = "acceptance")]
            Command::TestLegacyArticle { title, markdown } => {
                let g = self.ag()?;
                let saved = ArticleStore::open(self.article_path())?
                    .load()?
                    .ok_or("基线草稿缺失")?;
                if saved.title != title || saved.markdown != markdown {
                    return Err("基线测试内容与完整预览不一致".into());
                }
                let preview = self
                    .article_current
                    .as_ref()
                    .ok_or("基线测试必须先预览完整文章")?
                    .clone();
                let adapter = self.article_adapter()?;
                self.journal
                    .confirm(&g, &preview.id, &adapter, now())
                    .map_err(|_| "基线预览已失效")?;
                // Controlled benchmark without journal execution/reconciliation:
                // a naive retry after lost acknowledgement picks a fresh ID.
                // Only an opted-in test build can invoke this path.
                let mut ids = vec![];
                for _ in 0..2 {
                    let mut attempt = preview.clone();
                    attempt.id = new_id();
                    ids.push(attempt.id.clone());
                    let result = channel::publish(
                        &self.rt,
                        self.active(),
                        &adapter.room,
                        &attempt,
                        "m.room.message",
                    );
                    if result.is_ok() {
                        break;
                    }
                }
                std::fs::write(
                    self.root
                        .join(".run/acceptance/legacy-attempts.private.json"),
                    serde_json::to_vec(&json!({"ids":ids,"content":saved.action().payload}))
                        .unwrap(),
                )
                .map_err(|_| "基线证据不可保存")?;
                Ok("受控基线测试完成；需要核对 SDK 事件数，不能作为生产执行记录。".into())
            }
            #[cfg(feature = "acceptance")]
            command @ (Command::CollectEvidence | Command::CollectEvidenceSince(_)) => {
                self.g("read")?;
                let a = self.activity()?;
                let room = OwnedRoomId::try_from(a.room.as_str()).map_err(|_| "活动房间不合法")?;
                let anchor = match command {
                    Command::CollectEvidenceSince(id) => Some(id),
                    _ => None,
                };
                let values = official_sync::evidence_since(
                    &self.rt,
                    self.active(),
                    &room,
                    anchor.as_deref(),
                )?;
                let events=values.into_iter().filter(|v|matches!(v["type"].as_str(),Some("org.buwei.invitation"|"org.buwei.join"|"org.buwei.reply"|"org.buwei.cancel"|"m.room.message"|"org.buwei.activity"|"org.octosense.article"))).map(|v|json!({"event_id":v["event_id"],"sender":v["sender"],"type":v["type"],"origin_server_ts":v["origin_server_ts"],"content":v["content"]})).collect::<Vec<_>>();
                let evidence = json!({"collected_at_unix":now(),"complete_sdk_history":anchor.is_none(),"complete_since_anchor":anchor,"room":a.room,"events":events});
                let dir = self.root.join(".run/acceptance");
                std::fs::write(
                    dir.join("events.private.json"),
                    serde_json::to_vec_pretty(&evidence).map_err(|_| "证据格式不合法")?,
                )
                .map_err(|_| "私密证据目录不可写")?;
                Ok("完整 SDK 事件与服务端时间已保存到本机私密验收目录。".into())
            }
            Command::Authorize => {
                self.consent = Some(consent::Consent::prepare(self.actor(), now()));
                Ok("请核对下面的账号、权限范围和有效期，再确认授权。".into())
            }
            Command::ConfirmAuthorization(id) => {
                self.policies.clear();
                self.policy_preview = None;
                self.consent.as_ref().ok_or("请先查看授权范围")?.check(
                    &id,
                    &self.actor(),
                    now(),
                )?;
                self.consent = None;
                if let Some(g) = &self.grant {
                    g.revoke();
                }
                if let Some(g) = &self.article_grant {
                    g.revoke();
                }
                if let Some(g) = &self.sync_grant {
                    g.revoke();
                }
                let clock = now();
                self.authorized_until = clock + 3600;
                self.grant = Some(
                    self.authority
                        .grant(
                            "buwei",
                            &[
                                "create",
                                "invite",
                                "participate",
                                "model",
                                "read",
                                "share_card",
                            ],
                            clock,
                            3600,
                        )
                        .map_err(|_| "授权不可用")?,
                );
                self.article_grant = Some(
                    self.authority
                        .grant("buwei-article", &["create", "publish"], clock, 3600)
                        .map_err(|_| "文章授权不可用")?,
                );
                self.sync_grant = Some(
                    self.authority
                        .grant("buwei-sync", &["sync_state"], clock, 3600)
                        .map_err(|_| "同步授权不可用")?,
                );
                self.last_sync_status =
                    "已授权自动读取回复并同步活动状态，每 10 秒检查；尚未同步".into();
                let recent = self
                    .journal
                    .recent(self.grant.as_ref().unwrap(), now())
                    .map_err(|_| "历史回执不可用")?;
                self.current = recent
                    .iter()
                    .find(|o| o.action.permission == "invite")
                    .cloned();
                self.participant_current = recent
                    .iter()
                    .find(|o| o.action.permission == "participate")
                    .cloned();
                self.article_current = self
                    .journal
                    .recent(self.article_grant.as_ref().unwrap(), now())
                    .map_err(|_| "文章回执不可用")?
                    .into_iter()
                    .next();
                Ok("已授权当前真实账号。旧的未执行确认不会自动恢复，请重新预览。".into())
            }
            Command::Switch => {
                self.policies.clear();
                self.policy_preview = None;
                self.dated_suggestion = None;
                self.selected = !self.selected;
                self.authority.set_account(Some(&self.actor()));
                self.grant = None;
                self.article_grant = None;
                self.sync_grant = None;
                self.current = None;
                self.participant_current = None;
                self.article_current = None;
                self.consent = None;
                self.authorized_until = 0;
                self.last_sync_status = "账号已切换，自动同步停止".into();
                self.suggestion = None;
                self.note = None;
                self.explanation = None;
                Ok("账号已切换；旧授权与旧确认已失效。".into())
            }
            Command::Revoke => {
                self.policies.clear();
                self.policy_preview = None;
                self.dated_suggestion = None;
                self.dated_dialogue = None;
                if let Some(g) = &self.grant {
                    g.revoke();
                }
                if let Some(g) = &self.article_grant {
                    g.revoke();
                }
                if let Some(g) = &self.sync_grant {
                    g.revoke();
                }
                self.last_sync_status = "授权已撤销，自动同步停止".into();
                self.suggestion = None;
                self.note = None;
                self.explanation = None;
                self.consent = None;
                self.authorized_until = 0;
                Ok("当前账号的授权已撤销。".into())
            }
            Command::JoinRoom(room) => {
                #[cfg(feature = "full-host")]
                if rinx_bridge::official_mode() {
                    self.g("participate")?;
                    let room =
                        OwnedRoomId::try_from(room.trim()).map_err(|_| "请填写完整活动房间编号")?;
                    if let Ok(a) = self.activity() {
                        if a.room != room.as_str() {
                            return Err("本版只管理一场活动，请保留现有活动".into());
                        }
                    }
                    self.rt
                        .block_on(self.active().join_room_by_id(&room))
                        .map_err(|_| "尚不能加入；请组织者先邀请这个测试账号加入房间")?;
                    let a = official_sync::fetch(&self.rt, self.active(), &room)?;
                    if a.owner == self.actor() {
                        return Err("组织者请使用创建活动入口".into());
                    }
                    Store::open(self.state_path())?.cache_verified_snapshot(&a)?;
                    return Ok("已核验组织者并读取活动；本人登记和回复由组织者同步后生效。".into());
                }
                Err("房间接入用于正式 Rinx 模式".into())
            }
            Command::InviteMember(recipient) => {
                #[cfg(feature = "full-host")]
                if rinx_bridge::official_mode() {
                    self.g("invite")?;
                    let a = self.activity()?;
                    if self.actor() != a.owner {
                        return Err("只有组织者可以邀请房间成员".into());
                    }
                    let recipient = matrix_sdk::ruma::OwnedUserId::try_from(recipient.trim())
                        .map_err(|_| "请填写第二个测试账号的完整 Matrix ID")?;
                    if recipient.as_str() == a.owner {
                        return Err("需要另一个真实身份".into());
                    }
                    let room =
                        OwnedRoomId::try_from(a.room.as_str()).map_err(|_| "房间编号不可用")?;
                    let request=matrix_sdk::ruma::api::client::membership::invite_user::v3::Request::new(room,matrix_sdk::ruma::api::client::membership::invite_user::v3::InvitationRecipient::UserId(matrix_sdk::ruma::api::client::membership::invite_user::v3::InviteUserId::new(recipient)));
                    self.rt
                        .block_on(async { self.active().send(request).await })
                        .map_err(|_| "房间成员邀请结果待核实，请在 Rinx 核对后再操作")?;
                    return Ok("服务器已受理房间成员邀请；这不会分配活动席位。".into());
                }
                Err("成员邀请用于正式 Rinx 模式".into())
            }
            Command::SyncActivity => {
                #[cfg(feature = "full-host")]
                if rinx_bridge::official_mode() {
                    return self.sync_activity();
                }
                Ok("本机测试模式已经共用权威状态".into())
            }
            Command::Create {
                title,
                capacity,
                start,
                end,
            } => {
                let g = self.g("create")?;
                if self.actor() != account(&self.owner) {
                    return Err("请切换到组织者创建活动".into());
                }
                if Store::open(self.state_path())?.load()?.is_some() {
                    return Err("本版仅管理一场活动，原活动已保留".into());
                }
                let candidate = Activity::new(
                    self.actor(),
                    "!pending".into(),
                    title,
                    capacity,
                    start as u64,
                    end as u64,
                )?;
                let room = self
                    .rt
                    .block_on(async {
                        let mut r = create_room::v3::Request::new();
                        r.name = Some(candidate.title.clone());
                        if account(&self.owner) != account(&self.participant) {
                            r.invite
                                .push(self.participant.user_id().unwrap().to_owned());
                        }
                        self.owner.create_room(r).await
                    })
                    .map_err(|_| "活动房间创建结果待核实；请核对测试服务器")?
                    .room_id()
                    .to_owned();
                if account(&self.owner) != account(&self.participant) {
                    self.rt
                        .block_on(self.participant.join_room_by_id(&room))
                        .map_err(|_| "参与者尚未加入房间，请检查连接")?;
                }
                let a = Activity::new(
                    self.actor(),
                    room.to_string(),
                    candidate.title,
                    capacity,
                    start as u64,
                    end as u64,
                )?;
                Store::open(self.state_path())?.create(&g, &a, now())?;
                Ok("活动已建立。正式模式请先同步活动，邀请第二个账号加入房间，再由本人接入活动房间。".into())
            }
            Command::Join(p) => {
                self.prepare_participant(buwei_host_core::participation::Intent::Join {
                    preferences: p,
                })
            }
            Command::ConfirmParticipant(inputs) => {
                let g = self.g("participate")?;
                let op = self
                    .participant_current
                    .as_ref()
                    .ok_or("请先预览本人操作")?
                    .clone();
                let intent = buwei_host_core::participation::Intent::parse(&op.action)?;
                if let buwei_host_core::participation::Intent::Join { preferences } = intent {
                    if preferences != inputs {
                        return Err("时段或人数已修改，旧确认失效，请重新预览".into());
                    }
                }
                let adapter = self.participant_adapter()?;
                if op.status == Status::Prepared {
                    self.journal
                        .confirm(&g, &op.id, &adapter, now())
                        .map_err(|_| "本人确认已变化或过期，请重新预览")?;
                }
                self.participant_current = Some(
                    self.journal
                        .execute(&g, &op.id, &adapter, now())
                        .map_err(|_| "本人操作不能重复发送，请沿原编号核实")?,
                );
                Ok("本人操作回执已记录；服务端送达与组织者最终核验分别显示。".into())
            }
            Command::ReconcilePending => self.recover_pending(),
            Command::Prepare => {
                let g = self.g("invite")?;
                let a = self.activity()?;
                if self.actor() != a.owner {
                    return Err("邀请需要组织者身份".into());
                }
                let who = a
                    .candidate()
                    .ok_or("当前没有符合时段、人数和容量条件的候补")?;
                self.current=Some(self.journal.prepare(&g,Action{permission:"invite".into(),target:a.room.clone(),summary:format!("邀请 {who}；整组保留 {} 个名额；截止 {}",a.people.iter().find(|p|p.account==who).map(|p|p.preferences.group).unwrap_or(1),buwei_host_core::calendar::display(if a.metadata.is_some(){(now()+300).min(a.start)}else{now()+300})),payload:json!({"person":who,"until":if a.metadata.is_some(){(now()+300).min(a.start)}else{now()+300}})},a.revision,now(),120).map_err(|_|"无法生成预览")?);
                Ok("确切预览已生成，尚未发送。".into())
            }
            Command::Execute => {
                let g = self.g("invite")?;
                let id = self.current.as_ref().ok_or("请先预览邀请")?.id.clone();
                let adapter = self.adapter()?;
                if self.current.as_ref().unwrap().status == Status::Prepared {
                    self.journal
                        .confirm(&g, &id, &adapter, now())
                        .map_err(|_| "确认已失效，请重新预览")?;
                }
                self.current = Some(
                    self.journal
                        .execute(&g, &id, &adapter, now())
                        .map_err(|_| "操作不能重复发送；待核实结果请查询回执")?,
                );
                Ok("执行结果已记录；送达与本人接受分别显示。".into())
            }
            Command::Reconcile => {
                let g = self.g("invite")?;
                let id = self.current.as_ref().ok_or("尚无邀请记录")?.id.clone();
                let adapter = self.adapter()?;
                self.current = Some(
                    self.journal
                        .reconcile(&g, &id, &adapter, now())
                        .map_err(|_| "原回执暂不可核实，保留编号和名额")?,
                );
                Ok("沿原编号查询完成，没有重发。".into())
            }
            Command::Accept(accept) => {
                self.g("participate")?;
                let a = self.activity()?;
                let i = a.pending_invitation_for(&self.actor(), now())?;
                self.prepare_participant(buwei_host_core::participation::Intent::Reply {
                    invitation_id: i.operation_id.clone(),
                    invitation_event: i.server_event.clone().ok_or("邀请尚未核验送达")?,
                    accept,
                })
            }
            Command::Cancel => {
                self.prepare_participant(buwei_host_core::participation::Intent::Cancel)
            }
            Command::Expire => {
                self.g("read")?;
                if self.activity()?.owner != self.actor() {
                    return Err("过期处理由组织者管理".into());
                }
                #[cfg(feature = "full-host")]
                if rinx_bridge::official_mode() {
                    return self.sync_activity();
                }
                let a = self.activity()?;
                Store::open(self.state_path())?.update(a.revision, |a| {
                    a.expire(now());
                    Ok(())
                })?;
                Ok("已检查过期；不明发送继续保留名额".into())
            }
            Command::Suggest(text) => {
                let g = self.g("model")?;
                let a = self.activity()?;
                let (advice, reply) = model::recommend(&self.model, &g, &text)?;
                self.model_receipt(&reply, a.revision)?;
                let clarify = advice.needs_clarification;
                self.explanation = None;
                self.suggestion = Some((
                    self.actor(),
                    advice_digest(&text, a.revision),
                    a.revision,
                    advice,
                    vec![text],
                ));
                Ok(if clarify {
                    "请按列出的问题填写补充回答，再继续理解需求。"
                } else {
                    "建议已生成。核对后预览本人报名，再单独确认发送。"
                }
                .into())
            }
            Command::Clarify {
                requirement,
                answer,
            } => {
                let g = self.g("model")?;
                let a = self.activity()?;
                let (actor, _, revision, advice, turns) =
                    self.suggestion.as_ref().ok_or("请先生成需求草稿")?;
                if *actor != self.actor()
                    || *revision != a.revision
                    || turns.first() != Some(&requirement)
                {
                    return Err("原需求、账号或活动版本已变化，请重新生成建议".into());
                }
                if !advice.needs_clarification {
                    return Err("信息已完整，可以核对后预览报名；修改需求时请重新生成".into());
                }
                let mut turns = turns.clone();
                turns.push(answer.clone());
                let (draft, reply) = model::recommend_dialogue(&self.model, &g, &turns)?;
                self.model_receipt(&reply, a.revision)?;
                let ready = !draft.needs_clarification;
                self.explanation = None;
                self.suggestion = Some((
                    self.actor(),
                    advice_digest(&dialogue_binding(&requirement, &answer), a.revision),
                    a.revision,
                    draft,
                    turns,
                ));
                Ok(if ready {
                    "补充信息已合并。核对完整时段和人数后预览本人报名。"
                } else {
                    "仍有信息待补充，请查看问题；也可以修改原需求重新生成。"
                }
                .into())
            }
            Command::ApplySuggestion(text) => {
                self.g("participate")?;
                let a = self.activity()?;
                let (actor, digest, revision, advice, _) =
                    self.suggestion.as_ref().ok_or("请先生成建议")?;
                if *actor != self.actor()
                    || *revision != a.revision
                    || *digest != advice_digest(&text, a.revision)
                {
                    return Err("输入、账号或活动版本已变化，请重新生成建议".into());
                }
                if advice.needs_clarification {
                    return Err("建议需要补充信息，请先完善需求或手动填写时段".into());
                }
                let p = advice.preferences()?;
                self.suggestion = None;
                self.note = None;
                self.explanation = None;
                self.apply(Command::Join(p))
            }

            Command::Explain => {
                let g = self.g("model")?;
                let a = self.activity()?;
                let (text, reply) = model::explain(&self.model, &g, &a)?;
                self.model_receipt(&reply, a.revision)?;
                self.explanation = Some(text);
                Ok("匹配说明已生成。队列、时段和容量仍由业务规则核验。".into())
            }
            Command::GenerateNote => {
                let g = self.g("model")?;
                let a = self.activity()?;
                let (note, reply) = model::note(&self.model, &g, &a)?;
                self.model_receipt(&reply, a.revision)?;
                self.note = Some((self.actor(), a.revision, note));
                Ok("活动小记已生成待审草稿。核对正文后确认保存，再单独预览发布。".into())
            }
            Command::ApplyNote { title, markdown } => {
                self.ag()?;
                let a = self.activity()?;
                let (actor, revision, note) = self.note.as_ref().ok_or("请先生成活动小记")?;
                if *actor != self.actor()
                    || *revision != a.revision
                    || note.title != title
                    || note.markdown != markdown
                {
                    return Err("账号、活动或草稿已变化。请重新生成，或使用手动保存草稿。".into());
                }
                self.apply(Command::Draft { title, markdown })?;
                self.note = None;
                Ok("已确认并保存小记草稿，尚未发布。请继续预览完整文章。".into())
            }

            Command::Draft { title, markdown } => {
                let g = self.ag()?;
                let a = self.activity()?;
                let mut s = ArticleStore::open(self.article_path())?;
                if let Some(old) = s.load()? {
                    let actor = self.actor();
                    s.update(old.revision, |a| a.edit(&actor, title, markdown))?;
                } else {
                    s.create(
                        &g,
                        &Article::new(self.actor(), a.room, title, markdown)?,
                        now(),
                    )?;
                }
                Ok("完整文章草稿已保存；旧预览不能用于修改后的正文。".into())
            }
            Command::NewArticle { title, markdown } => {
                let g = self.ag()?;
                let a = self.activity()?;
                let mut store = ArticleStore::open(self.article_path())?;
                let previous = store.load()?.ok_or("第一篇文章请使用保存草稿")?;
                store.start_next(
                    &g,
                    previous.revision,
                    Article::new(self.actor(), a.room, title, markdown)?,
                    now(),
                )?;
                self.article_current = None;
                Ok("已保留原文章及服务端回执，建立下一篇草稿。请核对正文后重新预览。".into())
            }
            Command::PrepareArticle => {
                let g = self.ag()?;
                let a = ArticleStore::open(self.article_path())?
                    .load()?
                    .ok_or("请先保存文章草稿")?;
                self.article_current = Some(
                    self.journal
                        .prepare(&g, a.action(), a.revision, now(), 120)
                        .map_err(|_| "文章预览不可用")?,
                );
                Ok("文章确切预览已生成；完整标题和正文将在确认后发布。".into())
            }
            Command::PublishArticle { title, markdown } => {
                let g = self.ag()?;
                let saved = ArticleStore::open(self.article_path())?
                    .load()?
                    .ok_or("请先保存完整草稿")?;
                if saved.title != title || saved.markdown != markdown {
                    return Err("输入内容已变化，旧确认失效。请保存完整草稿并重新预览。".into());
                }
                let id = self
                    .article_current
                    .as_ref()
                    .ok_or("请先预览文章")?
                    .id
                    .clone();
                let adapter = self.article_adapter()?;
                if self.article_current.as_ref().unwrap().status == Status::Prepared {
                    self.journal
                        .confirm(&g, &id, &adapter, now())
                        .map_err(|_| "文章预览已失效")?;
                }
                self.article_current = Some(
                    self.journal
                        .execute(&g, &id, &adapter, now())
                        .map_err(|_| "文章不能重复发送，待核实结果请查询回执")?,
                );
                Ok("文章结果已记录；取得匹配服务端事件后才显示已发布。".into())
            }
            Command::ReconcileArticle => {
                let g = self.ag()?;
                let id = self
                    .article_current
                    .as_ref()
                    .ok_or("文章发布记录不存在")?
                    .id
                    .clone();
                let adapter = self.article_adapter()?;
                self.article_current = Some(
                    self.journal
                        .reconcile(&g, &id, &adapter, now())
                        .map_err(|_| "文章回执暂不可核实，原编号保留")?,
                );
                Ok("文章沿原编号核实完成，没有重新发布。".into())
            }
            Command::Fault => {
                self.fault = !self.fault;
                Ok("仅对本机测试服务丢弃本地回执，真实服务端发送仍执行。".into())
            }
            _ if community::is_community_command(&command) => unreachable!(),
            Command::ShowTodos => {
                self.assistance_route = Some(if self.is_authorized() { 0 } else { 4 });
                Ok("已打开待办；请先处理待核实操作，再确认新的建议。".into())
            }
            Command::Refresh => {
                #[cfg(feature = "full-host")]
                if rinx_bridge::official_mode() {
                    if let Ok(a) = self.activity() {
                        if self.actor() != a.owner {
                            self.g("read")?;
                            let room =
                                OwnedRoomId::try_from(a.room.as_str()).map_err(|_| "房间不合法")?;
                            let state = official_sync::fetch(&self.rt, self.active(), &room)?;
                            Store::open(self.state_path())?.cache_verified_snapshot(&state)?;
                        }
                    }
                }
                Ok("活动记录已刷新。".into())
            }
            _ => Err("操作入口不匹配".into()),
        }
    }
    #[cfg(feature = "full-host")]
    fn sync_activity(&mut self) -> Result<String> {
        self.g("read")?;
        let a = self.activity()?;
        if self.actor() != a.owner {
            return self
                .apply(Command::Refresh)
                .map(|_| "组织者活动状态已刷新".into());
        }
        self.g("invite")?;
        let room = OwnedRoomId::try_from(a.room.as_str()).map_err(|_| "活动房间不可用")?;
        official_sync::check_authority(&self.rt, self.active(), &a)?;
        // The expiry watermark is captured before reading. Replies accepted by
        // the server while paging cannot be released by a later local clock.
        let watermark = official_sync::server_watermark(self.active());
        let clock_captured = std::time::Instant::now();
        let mut store = Store::open(self.state_path())?;
        let checkpoint = store.sync_checkpoint(&a.room)?;
        let events =
            official_sync::timeline_since(&self.rt, self.active(), &room, checkpoint.as_deref())?;
        let mut accepted = 0;
        let mut rejected = 0;
        let last = events
            .last()
            .and_then(|v| v["event_id"].as_str())
            .map(str::to_owned);
        let reply_clock = watermark
            .as_ref()
            .map(|time| {
                time.saturating_add(5)
                    .saturating_add(clock_captured.elapsed().as_secs())
            })
            .unwrap_or_else(|_| now());
        for v in events {
            if !matches!(
                v["type"].as_str(),
                Some("org.buwei.join" | "org.buwei.reply" | "org.buwei.cancel")
            ) {
                continue;
            }
            let id = v["event_id"].as_str().ok_or("服务端编号缺失")?;
            match store.apply_verified_event(id, |a| {
                official_sync::apply_participant(a, room.as_str(), &v, reply_clock)
            })? {
                Some(true) => accepted += 1,
                Some(false) => rejected += 1,
                None => {}
            }
        }
        if let Some(id) = last {
            store.save_sync_checkpoint(&a.room, &id)?;
        }
        let current = store.load()?.ok_or("活动缺失")?;
        let mut expired = 0;
        if let Ok(watermark) = watermark.as_ref() {
            store.update(current.revision, |a| {
                expired = a.expire(*watermark);
                Ok(())
            })?;
        }
        let sg = self.sync_grant.as_ref().ok_or("同步授权不可用")?.clone();
        official_sync::publish(
            self.rt.clone(),
            &self.owner,
            self.state_path(),
            &sg,
            &mut self.journal,
        )?;
        let warning = watermark.err().map(|e| format!(" {e}")).unwrap_or_default();
        let result = format!(
            "活动同步已核验：有效 {accepted}，拒绝 {rejected}，过期释放 {expired}；进度已保存。{warning}"
        );
        self.last_sync_status = result.clone();
        Ok(result)
    }
    pub fn automatic_sync(&mut self) -> Option<View> {
        #[cfg(feature = "acceptance")]
        if acceptance::automatic_sync_paused(&self.root) {
            return None;
        }
        if !self.is_authorized() {
            return None;
        }
        #[cfg(feature = "full-host")]
        if rinx_bridge::official_mode() {
            return self.community_tick();
        }
        None
    }
    fn participant_adapter(&self) -> Result<participant::ParticipantAdapter> {
        let a = self.activity()?;
        Ok(participant::ParticipantAdapter {
            runtime: self.rt.clone(),
            client: self.active().clone(),
            room: OwnedRoomId::try_from(a.room.as_str()).map_err(|_| "房间编号不合法")?,
            state: self.state_path(),
            drop_ack: self.fault,
        })
    }
    fn prepare_participant(
        &mut self,
        intent: buwei_host_core::participation::Intent,
    ) -> Result<String> {
        let g = self.g("participate")?;
        let a = self.activity()?;
        intent.validate(&a, &self.actor(), now())?;
        let pending = self
            .journal
            .pending(&g, now())
            .map_err(|_| "待核实记录不可读取")?;
        if pending.iter().any(|o| o.action.target == a.room) {
            return Err("还有原编号待核实操作，请先查询回执，不会创建新交易".into());
        }
        let action = intent.action(&a);
        if let Some(op) = self
            .journal
            .recent(&g, now())
            .map_err(|_| "历史记录不可读取")?
            .into_iter()
            .find(|op| {
                op.revision == a.revision
                    && op.action == action
                    && (op.status == Status::Confirmed || Journal::preview_current(&g, op, now()))
            })
        {
            self.participant_current = Some(op);
            return Ok("沿用本人原操作编号。核对预览后确认；已送达操作不会重发。".into());
        }
        self.participant_current = Some(
            self.journal
                .prepare(&g, action, a.revision, now(), 120)
                .map_err(|_| "本人操作预览不可用")?,
        );
        Ok("本人操作已预览，尚未发送。请核对账号、活动和参数后确认。".into())
    }
    fn recover_pending(&mut self) -> Result<String> {
        let g = self.g("read")?;
        let ops = self
            .journal
            .pending(&g, now())
            .map_err(|_| "待核实记录不可读取")?;
        let mut restored = 0;
        let mut unresolved = 0;
        for op in ops {
            if self.activity().is_ok_and(|a| {
                if op.action.permission == "share_card" {
                    op.action.payload["card"]["room"] != a.room
                } else {
                    op.action.target != a.room
                }
            }) {
                continue;
            }
            let result = if op.action.permission == "participate" {
                let adapter = self.participant_adapter()?;
                self.journal.reconcile(&g, &op.id, &adapter, now())
            } else if op.action.permission == "invite" {
                let adapter = self.adapter()?;
                self.journal.reconcile(&g, &op.id, &adapter, now())
            } else if op.action.permission == "share_card" {
                let adapter = super::activity_card::CardAdapter {
                    runtime: self.rt.clone(),
                    client: self.active().clone(),
                    state: self.state_path(),
                };
                self.journal.reconcile(&g, &op.id, &adapter, now())
            } else {
                continue;
            };
            match result {
                Ok(o) => {
                    if o.status == Status::Confirmed {
                        restored += 1;
                    } else {
                        unresolved += 1;
                    }
                    if o.action.permission == "participate" {
                        self.participant_current = Some(o);
                    } else if o.action.permission == "share_card" {
                        self.share_current = Some(o);
                    } else {
                        self.current = Some(o);
                    }
                }
                Err(_) => unresolved += 1,
            }
        }
        let ag = self.ag()?;
        for op in self
            .journal
            .pending(&ag, now())
            .map_err(|_| "文章待核实记录不可读取")?
        {
            if self.activity().is_ok_and(|a| op.action.target != a.room) {
                continue;
            }
            let adapter = self.article_adapter()?;
            match self.journal.reconcile(&ag, &op.id, &adapter, now()) {
                Ok(o) => {
                    if o.status == Status::Confirmed {
                        restored += 1;
                    } else {
                        unresolved += 1;
                    }
                    self.article_current = Some(o);
                }
                Err(_) => unresolved += 1,
            }
        }
        Ok(format!(
            "沿原编号核实：恢复 {restored} 项，仍待核实 {unresolved} 项。没有重新发送。"
        ))
    }
    pub fn shutdown(&mut self) {
        self.analysis_until = 0;
        self.analysis_consent = None;
        self.automation_interlock.invalidate_all();
        self.dated_dialogue = None;
        self.dated_suggestion = None;
        self.policies.clear();
        self.policy_preview = None;
        self.consent = None;
        self.authorized_until = 0;
        if let Some(g) = &self.grant {
            g.revoke();
        }
        if let Some(g) = &self.article_grant {
            g.revoke();
        }
        if let Some(g) = &self.sync_grant {
            g.revoke();
        }
        #[cfg(feature = "full-host")]
        if rinx_bridge::official_mode() {
            let _ = rinx_bridge::record_status(&self.root, None, false, false);
        }
    }
    pub fn is_authorized(&self) -> bool {
        self.grant
            .as_ref()
            .is_some_and(|g| g.check("read", now()).is_ok())
    }
    #[cfg(feature = "acceptance")]
    pub fn acceptance_snapshot(&self) -> Value {
        let view = self.view();
        json!({"version":env!("CARGO_PKG_VERSION"),"collected_at_unix":now(),"account":self.actor(),"authorized":self.is_authorized(),"consent_id":self.consent.as_ref().map(|c|&c.id),"message":self.message,"sync_status":self.last_sync_status,"automation_status":view.automation_status,"reply_status":view.reply,"activity":self.activity().ok(),"invitation":self.current,"participant":self.participant_current,"article":self.article_current,"generated_note":self.note.as_ref().map(|(_,_,n)|json!({"title":n.title,"markdown":n.markdown})),"explanation":self.explanation,"policy_consent_id":view.policy_consent_id,"share":self.share_current,"contacts":self.contacts,"ai_result":self.ai_result,"activities":buwei_host_core::catalog::Catalog::open(&self.data).and_then(|c|c.list()).ok(),"intentions":self.intent_store().and_then(|s|s.load()).ok(),"assistance_cards":view.assistance_cards,"goal_form":view.goal_form,"goal_id":view.goal_id,"background":crate::host::background(),"background_paused":crate::host::paused(),"tray_hidden":crate::tray::hidden(),"analysis_consent_id":self.analysis_consent.as_ref().map(|c|&c.id)})
    }
    pub fn view(&self) -> View {
        let mut v = View {
            account: self.actor(),
            message: self.message.clone(),
            authorized: self
                .grant
                .as_ref()
                .is_some_and(|g| g.check("read", now()).is_ok()),
            fault: self.fault,
            organizer: std::env::var("BUWEI_PROFILE").as_deref() != Ok("participant"),
            sync_status: if self.is_authorized() {
                self.last_sync_status.clone()
            } else {
                "当前未授权，自动同步已停止".into()
            },
            ..Default::default()
        };
        v.consent_id = self.consent.as_ref().map(|p| p.id.clone());
        v.consent = if self.consent.is_some() {
            format!(
                "授权对象：{}\n补位：创建活动、邀请、本人登记与回复、读取记录、请求 AI 建议。\n自动读取回复并同步活动状态：应用打开期间每 10 秒检查；组织者同步已核验快照，参与者读取最终结果；自动补位需另外核对规则并启用；文章逐项确认发布。\n文章：保存草稿并在单独确认后发布。有效期：确认后 1 小时；授权预览 2 分钟内有效。",
                self.actor()
            )
        } else {
            "点击查看授权范围，再确认授权。".into()
        };
        v.expires = if v.authorized {
            format!(
                "授权剩余约 {} 分钟 · 到期北京时间 {:02}:{:02} · 可随时撤销",
                (self.authorized_until.saturating_sub(now()) + 59) / 60,
                ((self.authorized_until / 3600) + 8) % 24,
                (self.authorized_until / 60) % 60
            )
        } else {
            "当前未授权；重启、撤销或账号变化后需重新确认。".into()
        };
        match self.activity() {
            Ok(a) => {
                v.organizer = a.owner == self.actor();
                v.reply = match a.invitations.iter().rev().find(|i| {
                    i.recipient == self.actor()
                        && i.reply == buwei_host_core::Reply::Pending
                        && i.delivery == buwei_host_core::Delivery::Delivered
                }) {
                    Some(i) if now() < i.until => format!(
                        "本人邀请：{} · 还剩 {} 秒 · 北京时间 {:02}:{:02}:{:02} 截止。接受后等待组织者同步。",
                        i.recipient,
                        i.until.saturating_sub(now()),
                        ((i.until / 3600) + 8) % 24,
                        (i.until / 60) % 60,
                        i.until % 60
                    ),
                    Some(_) => {
                        "本人邀请已过期；请组织者检查过期并同步，再重新登记。迟到回复不会发送。"
                            .into()
                    }
                    None => "当前账号没有已送达且待回复的邀请。".into(),
                };
                if let Some(p) = a.people.iter().find(|p| p.account == self.actor()) {
                    if p.status != buwei_host_core::PersonStatus::Waiting {
                        v.reply = format!(
                            "组织者已核验本人最终结果：{}",
                            match p.status {
                                buwei_host_core::PersonStatus::Confirmed => "已接受，占位成功",
                                buwei_host_core::PersonStatus::Declined => "已拒绝，名额已释放",
                                buwei_host_core::PersonStatus::Cancelled => "已取消，名额已释放",
                                buwei_host_core::PersonStatus::Expired => "已过期，名额已释放",
                                _ => "候补中",
                            }
                        );
                    }
                }
                if let Some(op) = &self.participant_current {
                    if op.status == Status::Confirmed && op.action.permission == "participate" {
                        if let Ok(intent) =
                            buwei_host_core::participation::Intent::parse(&op.action)
                        {
                            let waiting = match intent {
                                buwei_host_core::participation::Intent::Join { .. } => {
                                    participant::join_awaits_projection(
                                        &a,
                                        &self.actor(),
                                        op.revision,
                                    )
                                }
                                buwei_host_core::participation::Intent::Reply {
                                    invitation_id,
                                    ..
                                } => a.invitations.iter().any(|i| {
                                    i.operation_id == invitation_id
                                        && i.reply == buwei_host_core::Reply::Pending
                                }),
                                buwei_host_core::participation::Intent::Cancel => {
                                    a.people.iter().any(|p| {
                                        p.account == self.actor()
                                            && p.status == buwei_host_core::PersonStatus::Confirmed
                                    })
                                }
                            };
                            if waiting {
                                v.reply="本人操作已送达服务器，等待组织者核验；以同步后的最终席位为准。".into();
                            }
                        }
                    }
                }
                v.activity = format!(
                    "{} · {}:00–{}:00\n容量 {} · 本人已接受 {} · 名额已保留 {} · 可用 {}\n活动对象：{} · 版本 {}",
                    a.title,
                    a.start,
                    a.end,
                    a.capacity,
                    a.confirmed(),
                    a.held(),
                    a.free(),
                    a.room,
                    a.revision
                );
                v.people=a.ordered_people().iter().enumerate().map(|(n,p)|{let status=match p.status{buwei_host_core::PersonStatus::Waiting=>"候补中",buwei_host_core::PersonStatus::Confirmed=>"本人已接受",buwei_host_core::PersonStatus::Declined=>"本人已拒绝",buwei_host_core::PersonStatus::Cancelled=>"本人已取消",buwei_host_core::PersonStatus::Expired=>"邀请已过期"};let delivery=a.invitations.iter().rev().find(|i|i.recipient==p.account).map(|i|format!("\n最近邀请：名额{} · 邀请{} · {}",if i.reply==buwei_host_core::Reply::Pending&&i.delivery!=buwei_host_core::Delivery::Rejected{"已保留"}else{"已结算"},match i.delivery{buwei_host_core::Delivery::Pending=>"待核验",buwei_host_core::Delivery::Unknown=>"结果待核实",buwei_host_core::Delivery::Delivered=>"已送达",buwei_host_core::Delivery::Rejected=>"发送被拒绝"},match i.reply{buwei_host_core::Reply::Pending=>"本人尚未接受",buwei_host_core::Reply::Accepted=>"本人已接受",buwei_host_core::Reply::Declined=>"本人已拒绝",buwei_host_core::Reply::Expired=>"本人未回复，已过期"})).unwrap_or_default();format!("当前列表第 {} 位 · 报名序号 {} · {} · {}\n账号：{}\n可用 {}–{} 点，{} 人{}",n+1,p.joined,p.name,status,p.account,p.preferences.earliest,p.preferences.latest,p.preferences.group,delivery)}).collect::<Vec<_>>().join("\n\n");
            }
            Err(_) => v.activity = "还没有活动。由组织者填写信息并创建。".into(),
        }
        fn describe(op: &Operation) -> String {
            let status = if matches!(op.status, Status::Prepared | Status::Queued)
                && now() >= op.expires_at
            {
                "确认已过期，请重新预览"
            } else {
                match op.status {
                    Status::Prepared => "等待确认（尚未发送）",
                    Status::Queued => "已确认，等待执行",
                    Status::Dispatching => "执行中，等待核验",
                    Status::Unknown => "结果待核实（禁止重发）",
                    Status::Confirmed => "服务端回执已核实",
                    Status::Failed => "操作失败",
                    Status::Cancelled => "操作已取消",
                    Status::Expired => "确认已过期",
                }
            };
            format!(
                "{}\n账号：{}\n对象：{}\n编号：{}\n摘要：{}\n状态：{}\n完整参数：{}",
                op.action.summary,
                op.account,
                op.action.target,
                op.id,
                op.digest,
                status,
                op.action.payload
            )
        }
        v.preview = self
            .current
            .as_ref()
            .map(describe)
            .unwrap_or_else(|| "尚未预览邀请。".into());
        v.participant_preview = self
            .participant_current
            .as_ref()
            .map(describe)
            .unwrap_or_else(|| "本人操作尚未预览。".into());
        v.participant_inputs = self.participant_current.as_ref().and_then(|op| {
            match buwei_host_core::participation::Intent::parse(&op.action).ok()? {
                buwei_host_core::participation::Intent::Join { preferences } => {
                    Some((op.id.clone(), preferences))
                }
                _ => None,
            }
        });
        v.history = self
            .grant
            .as_ref()
            .and_then(|g| self.journal.recent(g, now()).ok())
            .unwrap_or_default()
            .iter()
            .filter(|op| {
                self.activity().is_ok_and(|a| {
                    op.action.target == a.room || op.action.payload["card"]["room"] == a.room
                })
            })
            .take(12)
            .map(describe)
            .collect::<Vec<_>>()
            .join("\n\n");
        v.article = self
            .article_current
            .as_ref()
            .map(describe)
            .unwrap_or_else(|| "尚未预览文章。".into());
        v.draft = ArticleStore::open(self.article_path())
            .ok()
            .and_then(|s| s.load().ok().flatten())
            .map(|a| (a.title, a.markdown));
        self.community_view(&mut v);
        self.intention_view(&mut v);
        v.model_status = model::status(&self.root, &self.model);
        v.generated_note = self.note.as_ref().map(|(_, revision, n)| {
            (
                advice_digest(&format!("{}\n{}", n.title, n.markdown), *revision),
                n.title.clone(),
                n.markdown.clone(),
            )
        });
        v.advice = self
            .explanation
            .clone()
            .or_else(|| self.suggestion.as_ref().map(|(_, _, _, a, _)| a.display()))
            .unwrap_or_else(|| "AI 建议尚未生成。手动时段可直接使用。".into());
        v
    }
}

// Host-only SDK transaction keys bind identity, authoritative state version,
// event kind and full content. Retrying after an interrupted local update uses
// the same server transaction; UI data cannot choose an actor or transaction.
fn event_transaction(actor: &str, activity: &Activity, kind: &str, content: &Value) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(json!({"actor":actor,"room":activity.room,"revision":activity.revision,"kind":kind,"content":content}).to_string().as_bytes()))
}

#[path = "community.rs"]
pub(crate) mod community;
#[path = "intentions.rs"]
pub(crate) mod intentions;
