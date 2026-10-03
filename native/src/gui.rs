use super::Preferences;
use super::controller::{Command, View as HostView};
use makepad_app_module::{
    AppModule, ExecOutcome, InstanceHandles, InstanceParts, OpenSchema, ServiceExecutor,
    ValidatedOpen,
    makepad_ai_services::wire::{ServiceCall, ServiceManifest, ToolResult},
};
pub use makepad_widgets;
use makepad_widgets::*;
use std::sync::mpsc::{Receiver, SyncSender};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*
    let Text=Label{width: Fill height:Fit padding:0 draw_text.color:#x244139 draw_text.wrap:Words draw_text.text_style:theme.font_regular{font_size:13}}
    let Small=Text{draw_text.color:#x6a7b70 draw_text.text_style.font_size:11}
    let Heading=Text{draw_text.text_style:theme.font_bold{font_size:18}}
    let Action=Button{height:42 draw_text.text_style:theme.font_regular{font_size:13}}
    let Input=TextInput{width:Fill height:38 draw_text.text_style:theme.font_regular{font_size:13}}
    mod.widgets.BuWeiView = set_type_default() do #(BuWeiView::register_widget(vm)) {
        ..mod.widgets.RectView
        width:Fill height:Fill flow:Down padding:Inset{top:12 left:16 right:16 bottom:20} spacing:8
                Heading{text:"补位 · 让想来的人，刚好有位"}
                Small{text:#(concat!("v",env!("CARGO_PKG_VERSION")," · 社群活动助手"))}
                profile:=Small{text:"双账号联调窗口"}
                account:=Text{text:"请先在 Rinx 完成正式账号登录…"}
                View{width:Fill height:Fit flow:Right spacing:10
                    switch:=Action{text:"账号切换说明"}
                    authorize:=Action{text:"查看授权范围"}
                    confirm_authorization:=Action{text:"确认授权 1 小时"}
                    revoke:=Action{text:"撤销授权"}
                    refresh:=Action{text:"刷新记录"}
                }
                authorization:=Small{text:"点击查看授权范围，再确认授权。"}
                expires:=Small{text:"当前未授权"}
                sync_status:=Small{text:"自动同步尚未授权"}
                message:=Text{text:"正式服务器 https://matrix.rinx.chat；浏览器认证由本人完成。"}
                View{width:Fill height:40 flow:Right spacing:8
                    nav_activity:=Action{text:"我的活动"}
                    nav_confirm:=Action{text:"当前确认" visible:false}
                    nav_history:=Action{text:"活动记录"}
                    nav_article:=Action{text:"活动小记"}
                    nav_help:=Action{text:"设置与帮助"}
                }
                page_activity:=ScrollYView{width:Fill height:Fill flow:Down spacing:12 padding:16 show_bg:true draw_bg.color:#xf2f6ef
                    Heading{text:"我的活动"}
                    card_status:=Text{}
                    join_card:=Action{text:"确认加入并查看活动" visible:false}
                    home_current:=Text{}
                    home_status:=Text{}
                    reply_status:=Text{text:"当前没有邀请。"}
                    participant_actions:=View{width:Fill height:Fit flow:Right spacing:8
                        accept:=Action{text:"预览本人接受"} decline:=Action{text:"预览本人拒绝"} cancel:=Action{text:"预览本人取消"}
                    }
                    activity_0:=Action{width:Fill height:64 visible:false}
                    activity_1:=Action{width:Fill height:64 visible:false}
                    activity_2:=Action{width:Fill height:64 visible:false}
                    activity_3:=Action{width:Fill height:64 visible:false}
                    activity_4:=Action{width:Fill height:64 visible:false}
                    activity_pagination:=View{width:Fill height:Fit flow:Right spacing:8 activity_prev:=Action{text:"上一页"} activity_next:=Action{text:"下一页"}}
                    activity:=Text{text:"尚无活动"}
                    organizer_new_actions:=View{width:Fill height:Fit flow:Right spacing:8
                        new_activity:=Action{text:"新建活动"} copy_activity:=Action{text:"复制下一场"} archive_activity:=Action{text:"归档本场"}
                    }
                    organizer_setup:=View{width:Fill height:Fit flow:Down spacing:8
                        creation_form:=View{width:Fill height:Fit flow:Down spacing:8
                        Heading{text:"创建或复制下一场"}
                        View{width:Fill height:Fit flow:Right spacing:8
                            template_badminton:=Action{text:"羽毛球"} template_boardgame:=Action{text:"桌游"} template_reading:=Action{text:"读书会"}
                        }
                        title:=Input{text:"社群活动" empty_text:"活动名称"}
                        location:=Input{empty_text:"活动地点"}
                        description:=Input{height:70 is_multiline:true empty_text:"活动说明"}
                        View{width:Fill height:Fit flow:Right spacing:8
                            Small{text:"容量" width:40} capacity:=Input{text:"6" width:60}
                            Small{text:"开始" width:40} start:=Input{width:190 empty_text:"2026-10-10 19:30"}
                            Small{text:"结束" width:40} end:=Input{width:190 empty_text:"2026-10-10 21:30"}
                            create:=Action{text:"创建活动"}
                        }
                        recover_setup:=Action{text:"恢复待核实的活动创建"}
                        cancel_setup:=Action{text:"收起活动草稿"}
                        }
                        Heading{text:"分享本场活动"}
                        load_contacts:=Action{text:"选择 Rinx 联系人"}
                        contact_0:=Action{visible:false} contact_1:=Action{visible:false} contact_2:=Action{visible:false} contact_3:=Action{visible:false} contact_4:=Action{visible:false}
                        contact_pagination:=View{width:Fill height:Fit flow:Right spacing:8 contact_prev:=Action{text:"上一页联系人"} contact_next:=Action{text:"下一页联系人"}}
                        member:=Input{empty_text:"接收者（从联系人中选择）"}
                        View{width:Fill height:Fit flow:Right spacing:8
                            prepare_share:=Action{text:"预览活动卡片"} confirm_share:=Action{text:"确认分享与邀请"}
                        }
                        share_preview:=Text{}
                        invite_member:=Action{text:"邀请加入房间" visible:false}
                        Heading{text:"自动补位"}
                        automation_status:=Text{}
                        View{width:Fill height:Fit flow:Right spacing:8
                            Small{text:"邀请分钟"} invitation_minutes:=Input{text:"10" width:60}
                            Small{text:"发送时段"} quiet_start:=Input{text:"8" width:60} quiet_end:=Input{text:"22" width:60}
                            Small{text:"本次上限"} max_invitations:=Input{text:"30" width:60}
                        }
                        View{width:Fill height:Fit flow:Right spacing:8
                            preview_automation:=Action{text:"查看自动补位规则"} confirm_automation:=Action{text:"确认启用"} pause_automation:=Action{text:"暂停自动补位"}
                        }
                        policy_preview:=Text{}
                    }
                    participant_setup:=View{width:Fill height:Fit flow:Down spacing:8
                        Small{text:"从 Rinx 聊天打开活动卡片，即可用本人账号报名。"}
                        legacy_room_setup:=View{visible:false width:Fill height:Fit flow:Down
                            room:=Input{empty_text:"高级恢复：完整房间编号 !..."}
                            join_room:=Action{text:"本人加入活动房间"}
                        }
                        Small{text:"先核对本人可用时段，再预览报名。填写完整日期与时间，例如 2026-10-10 19:30。"}
                        View{width:Fill height:Fit flow:Right spacing:8
                            earliest:=Input{width:200 empty_text:"最早可用日期和时间"}
                            latest:=Input{width:200 empty_text:"最晚可用日期和时间"}
                            group:=Input{text:"1" width:90 empty_text:"同行人数"}
                            join:=Action{text:"预览本人报名"}
                        }
                    }
                    people:=Text{text:"尚无候补"}
                    Heading{text:"活动助手"}
                    requirement:=Input{height:70 is_multiline:true empty_text:"描述活动安排或本人报名意愿"}
                    clarification:=Input{height:55 is_multiline:true empty_text:"补充或更正时间、人数与地点"}
                    View{width:Fill height:Fit flow:Right spacing:8
                        create_with_ai:=Action{text:"一句话建活动"}
                        suggest:=Action{text:"理解报名意愿"} clarify:=Action{text:"合并补充回答"} apply_advice:=Action{text:"核对建议并预览报名"}
                        explain:=Action{text:"解释候补结果"}
                    }
                    advice:=Text{}
                    ai_result:=Text{}
                    question:=Input{empty_text:"问问这场活动，例如：几点开始、还有多少名额？"}
                    ask:=Action{text:"询问本场活动"}
                }
                page_confirm:=ScrollYView{visible:false width:Fill height:240 flow:Down spacing:12 padding:16 show_bg:true draw_bg.color:#xfffbf3
                    Heading{text:"核对并确认本次操作"}
                    Small{text:"核对本人账号、活动、时间与人数后确认。修改内容后请重新预览。"}
                    organizer_actions:=View{width:Fill height:Fit flow:Right spacing:8
                        sync_activity:=Action{text:"同步活动与回复"}
                        prepare:=Action{text:"预览邀请"} execute:=Action{text:"确认邀请"}
                        reconcile:=Action{text:"核实邀请回执"} expire:=Action{text:"检查过期邀请"}
                    }
                    preview:=Text{text:"尚未预览邀请。"}
                    participant_preview:=Text{text:"本人操作尚未预览。"}
                    participant_confirmation:=View{width:Fill height:Fit flow:Right
                        participant_confirm:=Action{text:"确认本人预览操作"}
                    }
                }
                page_history:=ScrollYView{visible:false width:Fill height:Fill flow:Down spacing:12 padding:16 show_bg:true draw_bg.color:#xf2f6ef
                    Heading{text:"活动记录与恢复"}
                    pending_reconcile:=Action{text:"沿原编号核实全部待恢复回执"}
                    Small{text:"待核实表示尚不能判断发送结果。重启并重新授权后查询原编号，不要创建相同的新操作。"}
                    history:=Text{text:"尚无执行记录"}
                }
                page_article:=ScrollYView{visible:false width:Fill height:Fill flow:Down spacing:12 padding:16 show_bg:true draw_bg.color:#xfffbf3
                    Heading{text:"活动小记"}
                    generate_note:=Action{text:"AI 生成去身份活动小记"}
                    article_title:=Input{empty_text:"文章标题"}
                    markdown:=Input{height:200 is_multiline:true empty_text:"完整 Markdown 正文"}
                    View{width:Fill height:Fit flow:Right spacing:8
                        apply_note:=Action{text:"确认并保存 AI 草稿"} draft:=Action{text:"保存手动草稿"}
                        new_article:=Action{text:"保留已发布文章并新建下一篇"}
                    }
                    View{width:Fill height:Fit flow:Right spacing:8
                        article_prepare:=Action{text:"预览文章"} publish:=Action{text:"确认发布"}
                        article_reconcile:=Action{text:"核实文章回执"}
                    }
                    article:=Text{text:"尚未预览文章"}
                }
                page_help:=ScrollYView{visible:false width:Fill height:Fill flow:Down spacing:12 padding:16 show_bg:true draw_bg.color:#xf2f6ef
                    Heading{text:"开始使用"}
                    Text{text:"1. 在 Rinx 完成本人登录，查看权限并确认授权。\n2. 组织者新建活动，选择联系人分享活动卡片。\n3. 成员从卡片进入活动，核对时段和同行人数后报名。\n4. 组织者核对规则并启用自动补位，成员亲自回复邀请。\n5. 不明结果在活动记录中沿原编号恢复。同场仅使用一个组织者宿主。"}
                    Heading{text:"模型设置"}
                    Small{text:"AI 建议会将填写的需求发送到 MiniMax；邀请和文章按完整预览单独确认。"}
                    model_status:=Text{text:"模型配置由宿主管理"}
                    configure_model:=Action{text:"填写或更新本机 MiniMax 密钥"}
                    Small{text:"活动助手可以理解日期和报名意愿。必要信息会先显示为可编辑草稿，由本人核对确认。"}
                    fault:=Action{text:"高级诊断" visible:false}
                }
    }
}
#[derive(Script, ScriptHook, Widget)]
pub struct BuWeiView {
    #[deref]
    view: View,
    #[rust]
    sender: Option<SyncSender<Command>>,
    #[rust]
    receiver: Option<Receiver<super::Result<HostView>>>,
    #[rust]
    busy: bool,
    #[rust]
    timer: Timer,
    #[rust]
    timer_started: bool,
    #[rust]
    scope: Option<makepad_app_module::InstanceScope>,
    #[rust]
    last_account: Option<String>,
    #[rust]
    last_participant_operation: Option<String>,
    #[rust]
    consent_id: Option<String>,
    #[rust]
    tab: u8,
    #[rust]
    last_note: Option<String>,
    #[rust]
    last_form: Option<String>,
    #[rust]
    activity_page: usize,
    #[rust]
    contact_page: usize,
    #[rust]
    activity_count: usize,
    #[rust]
    template: String,
    #[rust]
    policy_consent_id: Option<String>,
    #[rust]
    contacts: Vec<String>,
    #[rust]
    confirmation_ready: bool,
    #[rust]
    creating: bool,
    #[rust]
    pending_creation: bool,
    #[rust]
    pending_policy_pause: bool,
}
impl BuWeiView {
    fn display(&mut self, cx: &mut Cx, view: HostView) {
        let context = format!("{}:{}", view.account, view.activity_identity);
        if self.last_account.as_deref() != Some(context.as_str()) {
            let (title, markdown) = view.draft.clone().unwrap_or_default();
            self.view
                .text_input(cx, ids!(article_title))
                .set_text(cx, &title);
            self.view
                .text_input(cx, ids!(markdown))
                .set_text(cx, &markdown);
            self.last_participant_operation = None;
            self.last_note = None;
            self.last_account = Some(context);
        }
        if let Some((id, title, markdown)) = &view.generated_note {
            if self.last_note.as_ref() != Some(id) {
                self.view
                    .text_input(cx, ids!(article_title))
                    .set_text(cx, title);
                self.view
                    .text_input(cx, ids!(markdown))
                    .set_text(cx, markdown);
                self.last_note = Some(id.clone());
            }
        }
        // Apply host-validated preferences once per new preview. Polling must
        // preserve subsequent edits, which invalidate the old confirmation.
        if let Some((id, p)) = &view.participant_inputs {
            if self.last_participant_operation.as_ref() != Some(id) {
                self.view
                    .text_input(cx, ids!(earliest))
                    .set_text(cx, &buwei_host_core::calendar::display(p.earliest));
                self.view
                    .text_input(cx, ids!(latest))
                    .set_text(cx, &buwei_host_core::calendar::display(p.latest));
                self.view
                    .text_input(cx, ids!(group))
                    .set_text(cx, &p.group.to_string());
                self.last_participant_operation = Some(id.clone());
            }
        }
        if let Some(form) = &view.create_form {
            let key = serde_json::to_string(form).unwrap_or_default();
            if self.last_form.as_ref() != Some(&key) {
                self.view
                    .text_input(cx, ids!(title))
                    .set_text(cx, &form.title);
                self.view
                    .text_input(cx, ids!(capacity))
                    .set_text(cx, &form.capacity.to_string());
                self.view
                    .text_input(cx, ids!(start))
                    .set_text(cx, &form.start);
                self.view.text_input(cx, ids!(end)).set_text(cx, &form.end);
                self.view
                    .text_input(cx, ids!(location))
                    .set_text(cx, &form.location);
                self.view
                    .text_input(cx, ids!(description))
                    .set_text(cx, &form.description);
                self.template = form.template.clone();
                self.last_form = Some(key);
            }
        }
        if self.pending_creation {
            if view.success {
                self.creating = false;
            }
            self.pending_creation = false;
        }
        self.view
            .view(cx, ids!(creation_form))
            .set_visible(cx, self.creating || view.activity_list.is_empty());
        self.view
            .view(cx, ids!(organizer_new_actions))
            .set_visible(cx, view.organizer);
        self.activity_count = view.activity_list.len();
        self.view
            .view(cx, ids!(activity_pagination))
            .set_visible(cx, self.activity_count > 3);
        self.view.label(cx, ids!(home_current)).set_text(
            cx,
            &view.activity.lines().take(2).collect::<Vec<_>>().join("\n"),
        );
        self.view
            .label(cx, ids!(home_status))
            .set_visible(cx, view.organizer);
        self.view
            .label(cx, ids!(home_status))
            .set_text(cx, &view.automation_status);
        self.view
            .label(cx, ids!(reply_status))
            .set_visible(cx, !view.organizer);
        self.activity_page = self
            .activity_page
            .min(self.activity_count.saturating_sub(1) / 3);
        macro_rules! activity_button {
            ($id:ident,$n:expr) => {
                self.view.button(cx, ids!($id)).set_visible(
                    cx,
                    $n < 3 && view.activity_list.len() > self.activity_page * 3 + $n,
                );
                if let Some(text) = view.activity_list.get(self.activity_page * 3 + $n) {
                    self.view.button(cx, ids!($id)).set_text(cx, text);
                }
            };
        }
        activity_button!(activity_0, 0);
        activity_button!(activity_1, 1);
        activity_button!(activity_2, 2);
        activity_button!(activity_3, 3);
        activity_button!(activity_4, 4);
        self.contacts = view.contacts.clone();
        self.view
            .view(cx, ids!(contact_pagination))
            .set_visible(cx, self.contacts.len() > 5);
        macro_rules! contact_button {
            ($id:ident,$n:expr) => {
                self.view
                    .button(cx, ids!($id))
                    .set_visible(cx, self.contacts.len() > self.contact_page * 5 + $n);
                if let Some(text) = self.contacts.get(self.contact_page * 5 + $n) {
                    self.view.button(cx, ids!($id)).set_text(cx, text);
                }
            };
        }
        contact_button!(contact_0, 0);
        contact_button!(contact_1, 1);
        contact_button!(contact_2, 2);
        contact_button!(contact_3, 3);
        contact_button!(contact_4, 4);
        self.policy_consent_id = view.policy_consent_id.clone();
        self.confirmation_ready = view.confirmation_ready;
        self.view
            .label(cx, ids!(policy_preview))
            .set_text(cx, &view.policy_preview);
        self.view
            .label(cx, ids!(automation_status))
            .set_text(cx, &view.automation_status);
        self.view
            .label(cx, ids!(share_preview))
            .set_text(cx, &view.share_preview);
        self.view
            .label(cx, ids!(card_status))
            .set_text(cx, &view.card_status);
        self.view
            .label(cx, ids!(card_status))
            .set_visible(cx, !view.card_status.is_empty());
        self.view
            .button(cx, ids!(join_card))
            .set_visible(cx, !view.card_status.is_empty());
        self.view
            .label(cx, ids!(ai_result))
            .set_text(cx, &view.ai_result);
        self.view.label(cx, ids!(profile)).set_text(
            cx,
            if std::env::var("BUWEI_PROFILE").as_deref() == Ok("participant") {
                "参与者窗口 · 请登录本人 Rinx 账号"
            } else {
                "组织者窗口 · 登录身份由 Rinx 核验"
            },
        );
        self.view.label(cx, ids!(account)).set_text(
            cx,
            &format!(
                "当前真实账号：{} · {}",
                view.account,
                if view.authorized {
                    "已授权"
                } else {
                    "未授权"
                }
            ),
        );
        self.consent_id = view.consent_id.clone();
        self.view
            .label(cx, ids!(authorization))
            .set_text(cx, &view.consent);
        self.view
            .label(cx, ids!(expires))
            .set_text(cx, &view.expires);
        self.view
            .label(cx, ids!(activity))
            .set_text(cx, &view.activity);
        self.view.label(cx, ids!(people)).set_text(cx, &view.people);
        self.view
            .label(cx, ids!(sync_status))
            .set_text(cx, &view.sync_status);
        self.view
            .label(cx, ids!(model_status))
            .set_text(cx, &view.model_status);
        self.view
            .label(cx, ids!(authorization))
            .set_visible(cx, view.consent_id.is_some());
        self.view
            .label(cx, ids!(participant_preview))
            .set_visible(cx, !view.organizer);
        self.view
            .label(cx, ids!(preview))
            .set_visible(cx, view.organizer);
        self.view
            .view(cx, ids!(participant_confirmation))
            .set_visible(cx, !view.organizer);
        self.view.button(cx, ids!(nav_activity)).set_text(
            cx,
            if view.organizer {
                "我的活动"
            } else {
                "我的报名与邀请"
            },
        );
        self.view
            .button(cx, ids!(create_with_ai))
            .set_visible(cx, view.organizer);
        self.view
            .button(cx, ids!(suggest))
            .set_visible(cx, !view.organizer);
        self.view
            .button(cx, ids!(clarify))
            .set_visible(cx, !view.organizer);
        self.view
            .button(cx, ids!(apply_advice))
            .set_visible(cx, !view.organizer);
        self.pages(cx);
        self.view
            .view(cx, ids!(organizer_actions))
            .set_visible(cx, view.organizer);
        self.view
            .view(cx, ids!(organizer_setup))
            .set_visible(cx, view.organizer);
        self.view
            .view(cx, ids!(participant_actions))
            .set_visible(cx, !view.organizer);
        self.view
            .view(cx, ids!(participant_setup))
            .set_visible(cx, !view.organizer);
        self.view
            .label(cx, ids!(participant_preview))
            .set_text(cx, &view.participant_preview);
        self.view
            .label(cx, ids!(history))
            .set_text(cx, &view.history);
        self.view
            .label(cx, ids!(reply_status))
            .set_text(cx, &view.reply);
        self.view
            .label(cx, ids!(preview))
            .set_text(cx, &view.preview);
        self.view
            .label(cx, ids!(article))
            .set_text(cx, &view.article);
        self.view.label(cx, ids!(advice)).set_text(cx, &view.advice);
        self.view
            .label(cx, ids!(message))
            .set_text(cx, &view.message);
        self.view.button(cx, ids!(fault)).set_text(
            cx,
            if view.fault {
                "丢弃本地回执：开"
            } else {
                "丢弃本地回执：关"
            },
        );
        self.view.redraw(cx);
    }
    fn pages(&mut self, cx: &mut Cx) {
        self.view
            .view(cx, ids!(page_activity))
            .set_visible(cx, self.tab == 0 || self.tab == 1);
        self.view.view(cx, ids!(page_confirm)).set_visible(
            cx,
            (self.tab == 0 || self.tab == 1) && self.confirmation_ready,
        );
        self.view
            .view(cx, ids!(page_history))
            .set_visible(cx, self.tab == 2);
        self.view
            .view(cx, ids!(page_article))
            .set_visible(cx, self.tab == 3);
        self.view
            .view(cx, ids!(page_help))
            .set_visible(cx, self.tab == 4);
        self.view.redraw(cx);
    }
    fn send(&mut self, cx: &mut Cx, command: Command) {
        if self.busy {
            return;
        }

        if let Some(tx) = &self.sender {
            if tx.try_send(command).is_ok() {
                self.busy = true;
                self.view
                    .label(cx, ids!(message))
                    .set_text(cx, "正在处理，请等待结果。重复点击不会重复发送。");
                self.view.redraw(cx);
            }
        }
    }
}
impl Widget for BuWeiView {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if !self.timer_started {
            self.timer = cx.start_interval(0.25);
            self.timer_started = true;
            if let Some(scope) = self.scope {
                super::host::timer(scope, self.timer);
            }
        }
        if self.timer.is_event(event).is_some() {
            let updates = self
                .receiver
                .as_ref()
                .map(|r| r.try_iter().collect::<Vec<_>>())
                .unwrap_or_default();
            for update in updates {
                self.busy = false;
                match update {
                    Ok(view) => self.display(cx, view),
                    Err(message) => {
                        self.view.label(cx, ids!(message)).set_text(cx, &message);
                        self.view.redraw(cx);
                    }
                }
            }
            if self.pending_policy_pause {
                if let Some(tx) = &self.sender {
                    if tx.try_send(Command::PauseAutomation).is_ok() {
                        self.pending_policy_pause = false;
                    }
                }
            }
        }
        if let Event::Actions(actions) = event {
            if [
                ids!(invitation_minutes),
                ids!(quiet_start),
                ids!(quiet_end),
                ids!(max_invitations),
            ]
            .iter()
            .any(|path| self.view.text_input(cx, *path).changed(actions).is_some())
            {
                self.policy_consent_id = None;
                self.pending_policy_pause = true;
                self.view.label(cx, ids!(message)).set_text(
                    cx,
                    "规则已修改，正在暂停自动补位；请重新查看规则并确认。已发出的邀请继续核实。",
                );
            }
            macro_rules! clicked {
                ($id:ident) => {
                    self.view.button(cx, ids!($id)).clicked(actions)
                };
            }
            if let Some(tab) = if clicked!(nav_activity) {
                Some(0)
            } else if clicked!(nav_confirm) {
                Some(1)
            } else if clicked!(nav_history) {
                Some(2)
            } else if clicked!(nav_article) {
                Some(3)
            } else if clicked!(nav_help) {
                Some(4)
            } else {
                None
            } {
                self.tab = tab;
                self.pages(cx);
                return;
            }
            macro_rules! text {
                ($id:ident) => {
                    self.view.text_input(cx, ids!($id)).text()
                };
            }
            if clicked!(new_activity) {
                self.creating = true;
                self.view
                    .view(cx, ids!(creation_form))
                    .set_visible(cx, true);
                return;
            }
            if clicked!(cancel_setup) {
                self.creating = false;
                self.view
                    .view(cx, ids!(creation_form))
                    .set_visible(cx, false);
                return;
            }
            if clicked!(template_badminton)
                || clicked!(template_boardgame)
                || clicked!(template_reading)
                || clicked!(copy_activity)
                || clicked!(create_with_ai)
            {
                self.creating = true;
            }
            if clicked!(create) {
                self.pending_creation = true;
            }
            let parse = |value: String| {
                value
                    .parse::<u8>()
                    .map_err(|_| "请填写正确的人数或规则数值")
            };
            let time = |value: String| {
                if value.contains('-') {
                    buwei_host_core::calendar::parse(&value)
                } else {
                    value
                        .strip_suffix(":00")
                        .unwrap_or(&value)
                        .parse::<u64>()
                        .map_err(|_| "请填写完整日期和时间".into())
                }
            };
            if clicked!(activity_prev) || clicked!(activity_next) {
                if clicked!(activity_prev) {
                    self.activity_page = self.activity_page.saturating_sub(1);
                } else {
                    self.activity_page =
                        (self.activity_page + 1).min(self.activity_count.saturating_sub(1) / 3);
                }
                self.send(cx, Command::Refresh);
                return;
            }
            if clicked!(contact_prev) || clicked!(contact_next) {
                if clicked!(contact_prev) {
                    self.contact_page = self.contact_page.saturating_sub(1);
                } else {
                    self.contact_page =
                        (self.contact_page + 1).min(self.contacts.len().saturating_sub(1) / 5);
                }
                self.send(cx, Command::Refresh);
                return;
            }
            if let Some(index) = if clicked!(activity_0) {
                Some(0)
            } else if clicked!(activity_1) {
                Some(1)
            } else if clicked!(activity_2) {
                Some(2)
            } else if clicked!(activity_3) {
                Some(3)
            } else if clicked!(activity_4) {
                Some(4)
            } else {
                None
            } {
                self.send(cx, Command::SelectActivity(self.activity_page * 3 + index));
                return;
            }
            if let Some(index) = if clicked!(contact_0) {
                Some(0)
            } else if clicked!(contact_1) {
                Some(1)
            } else if clicked!(contact_2) {
                Some(2)
            } else if clicked!(contact_3) {
                Some(3)
            } else if clicked!(contact_4) {
                Some(4)
            } else {
                None
            } {
                if let Some(id) = self.contacts.get(self.contact_page * 5 + index) {
                    self.view.text_input(cx, ids!(member)).set_text(cx, id);
                }
                return;
            }
            let settings = |a: String,
                            b: String,
                            c: String,
                            d: String|
             -> super::Result<buwei_host_core::automation::Settings> {
                Ok(buwei_host_core::automation::Settings {
                    invitation_minutes: parse(a)?,
                    quiet_start: parse(b)?,
                    quiet_end: parse(c)?,
                    max_invitations: parse(d)?,
                })
            };
            let command: super::Result<Option<Command>> = (|| {
                Ok(if clicked!(authorize) {
                    Some(Command::Authorize)
                } else if clicked!(confirm_authorization) {
                    Some(Command::ConfirmAuthorization(
                        self.consent_id.clone().ok_or("请先查看授权范围")?,
                    ))
                } else if clicked!(switch) {
                    Some(Command::Switch)
                } else if clicked!(revoke) {
                    Some(Command::Revoke)
                } else if clicked!(refresh) {
                    Some(Command::Refresh)
                } else if clicked!(invite_member) {
                    Some(Command::InviteMember(text!(member)))
                } else if clicked!(sync_activity) {
                    Some(Command::SyncActivity)
                } else if clicked!(join_room) {
                    Some(Command::JoinRoom(text!(room)))
                } else if clicked!(create) {
                    Some(Command::CreateDated(
                        super::controller::community::ActivityForm {
                            title: text!(title),
                            capacity: parse(text!(capacity))?,
                            start: text!(start),
                            end: text!(end),
                            template: if self.template.is_empty() {
                                "custom".into()
                            } else {
                                self.template.clone()
                            },
                            location: text!(location),
                            description: text!(description),
                        },
                    ))
                } else if clicked!(template_badminton) {
                    Some(Command::UseTemplate("badminton".into()))
                } else if clicked!(template_boardgame) {
                    Some(Command::UseTemplate("boardgame".into()))
                } else if clicked!(template_reading) {
                    Some(Command::UseTemplate("reading".into()))
                } else if clicked!(copy_activity) {
                    Some(Command::CopyActivity)
                } else if clicked!(archive_activity) {
                    Some(Command::ArchiveActivity)
                } else if clicked!(recover_setup) {
                    Some(Command::RecoverSetup)
                } else if clicked!(load_contacts) {
                    Some(Command::LoadContacts)
                } else if clicked!(prepare_share) {
                    Some(Command::PrepareShare(text!(member)))
                } else if clicked!(confirm_share) {
                    Some(Command::ConfirmShare(text!(member)))
                } else if clicked!(join_card) {
                    Some(Command::JoinCard)
                } else if clicked!(preview_automation) {
                    Some(Command::PreviewAutomation(settings(
                        text!(invitation_minutes),
                        text!(quiet_start),
                        text!(quiet_end),
                        text!(max_invitations),
                    )?))
                } else if clicked!(confirm_automation) {
                    Some(Command::ConfirmAutomation {
                        id: self
                            .policy_consent_id
                            .clone()
                            .ok_or("请先查看自动补位规则")?,
                        settings: settings(
                            text!(invitation_minutes),
                            text!(quiet_start),
                            text!(quiet_end),
                            text!(max_invitations),
                        )?,
                    })
                } else if clicked!(pause_automation) {
                    Some(Command::PauseAutomation)
                } else if clicked!(create_with_ai) {
                    Some(Command::CreateWithAi(text!(requirement)))
                } else if clicked!(ask) {
                    Some(Command::Ask(text!(question)))
                } else if clicked!(join) {
                    Some(Command::Join(Preferences {
                        earliest: time(text!(earliest))?,
                        latest: time(text!(latest))?,
                        group: parse(text!(group))?,
                    }))
                } else if clicked!(participant_confirm) {
                    Some(Command::ConfirmParticipant(Preferences {
                        earliest: time(text!(earliest))?,
                        latest: time(text!(latest))?,
                        group: parse(text!(group))?,
                    }))
                } else if clicked!(pending_reconcile) {
                    Some(Command::ReconcilePending)
                } else if clicked!(prepare) {
                    Some(Command::Prepare)
                } else if clicked!(execute) {
                    Some(Command::Execute)
                } else if clicked!(reconcile) {
                    Some(Command::Reconcile)
                } else if clicked!(accept) {
                    Some(Command::Accept(true))
                } else if clicked!(decline) {
                    Some(Command::Accept(false))
                } else if clicked!(cancel) {
                    Some(Command::Cancel)
                } else if clicked!(expire) {
                    Some(Command::Expire)
                } else if clicked!(suggest) {
                    let requirement = text!(requirement);
                    self.view
                        .text_input(cx, ids!(clarification))
                        .set_text(cx, "");
                    Some(Command::SuggestDated {
                        requirement,
                        answer: String::new(),
                    })
                } else if clicked!(clarify) {
                    Some(Command::SuggestDated {
                        requirement: text!(requirement),
                        answer: text!(clarification),
                    })
                } else if clicked!(apply_advice) {
                    Some(Command::ApplyDatedSuggestion {
                        requirement: text!(requirement),
                        answer: text!(clarification),
                    })
                } else if clicked!(explain) {
                    Some(Command::Explain)
                } else if clicked!(generate_note) {
                    Some(Command::GenerateNote)
                } else if clicked!(apply_note) {
                    Some(Command::ApplyNote {
                        title: text!(article_title),
                        markdown: text!(markdown),
                    })
                } else if clicked!(configure_model) {
                    Some(Command::ConfigureModel)
                } else if clicked!(draft) {
                    Some(Command::Draft {
                        title: text!(article_title),
                        markdown: text!(markdown),
                    })
                } else if clicked!(new_article) {
                    Some(Command::NewArticle {
                        title: text!(article_title),
                        markdown: text!(markdown),
                    })
                } else if clicked!(article_prepare) {
                    Some(Command::PrepareArticle)
                } else if clicked!(publish) {
                    Some(Command::PublishArticle {
                        title: text!(article_title),
                        markdown: text!(markdown),
                    })
                } else if clicked!(article_reconcile) {
                    Some(Command::ReconcileArticle)
                } else if clicked!(fault) {
                    Some(Command::Fault)
                } else {
                    None
                })
            })();
            match command {
                Ok(Some(command)) => self.send(cx, command),
                Err(message) => {
                    self.view.label(cx, ids!(message)).set_text(cx, &message);
                    self.view.redraw(cx);
                }
                _ => {}
            }
        }
        self.view.handle_event(cx, event, scope);
        if matches!(event, Event::Scroll(_)) {
            self.view.redraw(cx);
        }
    }
}

pub struct BuWeiModule;
pub static MODULE: BuWeiModule = BuWeiModule;
impl AppModule for BuWeiModule {
    fn id(&self) -> &'static str {
        "buwei"
    }
    fn label(&self) -> &'static str {
        "补位"
    }
    fn register(&self, vm: &mut ScriptVm) {
        script_mod(vm);
    }
    fn open_schema(&self) -> OpenSchema {
        OpenSchema::new(1)
    }
    fn capabilities(&self) -> &'static [&'static str] {
        &["storage", "model", "matrix"]
    }
    fn create(
        &self,
        vm: &mut ScriptVm,
        _open: ValidatedOpen,
        handles: InstanceHandles,
    ) -> InstanceParts {
        let scope = handles.scope;
        let (sender, receiver) = super::host::open(scope);
        let value = script_eval!(vm,{use mod.widgets.* BuWeiView{}});
        let root = WidgetRef::script_from_value(vm, value);
        vm.with_cx_mut(|cx| {
            if let Some(mut view) = root.borrow_mut::<BuWeiView>() {
                view.sender = Some(sender);
                view.receiver = Some(receiver);
                view.busy = true;
                view.scope = Some(scope);
            }
            root.redraw(cx);
        });
        let closing_root = root.clone();
        InstanceParts {
            root,
            executor: Box::new(BuWeiExecutor),
            shutdown: Box::new(move |vm| {
                super::host::close(scope);
                vm.with_cx_mut(|cx| {
                    if let Some(mut view) = closing_root.borrow_mut::<BuWeiView>() {
                        if view.timer_started {
                            cx.stop_timer(view.timer);
                            view.timer_started = false;
                        }
                        view.sender = None;
                        view.receiver = None;
                    }
                });
            }),
        }
    }
}
struct BuWeiExecutor;
impl ServiceExecutor for BuWeiExecutor {
    fn manifest(&self) -> ServiceManifest {
        ServiceManifest::new("buwei", "补位", "宿主授权与回执应用；发送需要本人确认。")
    }
    fn execute(&mut self, _cx: &mut Cx, call: &ServiceCall) -> ExecOutcome {
        ExecOutcome::Done(ToolResult::unavailable(
            &call.call_id,
            "请在宿主界面预览并确认操作",
        ))
    }
}
