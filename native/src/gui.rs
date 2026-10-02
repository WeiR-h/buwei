pub use makepad_widgets;
use makepad_widgets::*;
use super::controller::{Command,View as HostView};
use super::Preferences;
use std::sync::mpsc::{SyncSender,Receiver};
use makepad_app_module::{AppModule,ExecOutcome,InstanceHandles,InstanceParts,OpenSchema,ServiceExecutor,ValidatedOpen,makepad_ai_services::wire::{ServiceCall,ServiceManifest,ToolResult}};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*
    let Text=Label{width: Fill height:Fit padding:0 draw_text.color:#x244139 draw_text.wrap:Words draw_text.text_style:theme.font_regular{font_size:13}}
    let Small=Text{draw_text.color:#x6a7b70 draw_text.text_style.font_size:11}
    let Heading=Text{draw_text.text_style:theme.font_bold{font_size:18}}
    let Action=Button{height:40 draw_text.text_style:theme.font_regular{font_size:13}}
    let Input=TextInput{width:Fill height:38 draw_text.text_style:theme.font_regular{font_size:13}}
    mod.widgets.BuWeiView = set_type_default() do #(BuWeiView::register_widget(vm)) {
        ..mod.widgets.RectView
        width:Fill height:Fill flow:Down padding:Inset{top:12 left:16 right:16 bottom:20} spacing:8
                Heading{text:"补位 · 让想来的人，刚好有位"}
                Small{text:#(concat!("v",env!("CARGO_PKG_VERSION")," · OctoSense 官方宿主 · Rinx 真实身份与服务端回执"))}
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
                    nav_activity:=Action{text:"活动与候补"}
                    nav_confirm:=Action{text:"动作确认"}
                    nav_history:=Action{text:"执行记录"}
                    nav_article:=Action{text:"文章"}
                    nav_help:=Action{text:"帮助与模型"}
                }
                page_activity:=ScrollYView{width:Fill height:Fill flow:Down spacing:12 padding:16 show_bg:true draw_bg.color:#xf2f6ef
                    Heading{text:"活动与候补"}
                    activity:=Text{text:"尚无活动"}
                    organizer_setup:=View{width:Fill height:Fit flow:Down spacing:8
                        title:=Input{text:"周末活动" empty_text:"活动名称"}
                        View{width:Fill height:Fit flow:Right spacing:8
                            Small{text:"容量" width:40} capacity:=Input{text:"1" width:60}
                            Small{text:"开始" width:40} start:=Input{text:"19" width:60}
                            Small{text:"结束" width:40} end:=Input{text:"21" width:60}
                            create:=Action{text:"创建活动"}
                        }
                        member:=Input{empty_text:"参与者的完整 Rinx 账号"}
                        invite_member:=Action{text:"邀请该账号加入活动房间"}
                    }
                    participant_setup:=View{width:Fill height:Fit flow:Down spacing:8
                        room:=Input{empty_text:"组织者提供的完整房间编号 !..."}
                        join_room:=Action{text:"本人加入活动房间"}
                        Small{text:"先核对本人可用时段，再预览报名。小时按 0–24 填写。"}
                        View{width:Fill height:Fit flow:Right spacing:8
                            earliest:=Input{text:"17" width:90 empty_text:"最早小时"}
                            latest:=Input{text:"23" width:90 empty_text:"最晚小时"}
                            group:=Input{text:"1" width:90 empty_text:"同行人数"}
                            join:=Action{text:"预览本人报名"}
                        }
                    }
                    reply_status:=Text{text:"当前没有邀请。"}
                    participant_actions:=View{width:Fill height:Fit flow:Right spacing:8
                        accept:=Action{text:"预览本人接受"} decline:=Action{text:"预览本人拒绝"} cancel:=Action{text:"预览本人取消"}
                    }
                    people:=Text{text:"尚无候补"}
                }
                page_confirm:=ScrollYView{visible:false width:Fill height:Fill flow:Down spacing:12 padding:16 show_bg:true draw_bg.color:#xfffbf3
                    Heading{text:"动作确认"}
                    Small{text:"核对账号、对象、操作编号和完整内容后确认。内容变化时重新预览。"}
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
                    Heading{text:"执行记录与恢复"}
                    pending_reconcile:=Action{text:"沿原编号核实全部待恢复回执"}
                    Small{text:"待核实表示尚不能判断发送结果。重启并重新授权后查询原编号，不要创建相同的新操作。"}
                    history:=Text{text:"尚无执行记录"}
                }
                page_article:=ScrollYView{visible:false width:Fill height:Fill flow:Down spacing:12 padding:16 show_bg:true draw_bg.color:#xfffbf3
                    Heading{text:"文章确认发布"}
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
                    Text{text:"1. 在底部 Rinx 窗口完成本人登录，再回到补位。\n2. 查看授权范围并确认；关闭、撤销或到期会停止同步。\n3. 组织者创建活动并邀请房间成员；参与者加入房间并预览报名。\n4. 每项发送都在动作确认页核对。打开期间自动同步，送达与占位分别显示。\n5. 不明结果在执行记录页沿原编号恢复。只运行一个组织者宿主。"}
                    Heading{text:"模型与开发预算"}
                    Small{text:"AI 建议会将填写的需求发送到 MiniMax；邀请和文章按完整预览单独确认。"}
                    model_status:=Text{text:"模型配置由宿主管理"}
                    configure_model:=Action{text:"填写或更新本机 MiniMax 密钥"}
                    requirement:=Input{height:85 is_multiline:true text:"我今晚七点到九点有空，一个人参加。"}
                    View{width:Fill height:Fit flow:Right spacing:8
                        suggest:=Action{text:"生成本人偏好建议"} apply_advice:=Action{text:"核对建议并预览报名"}
                        explain:=Action{text:"解释候补匹配规则"}
                    }
                    advice:=Text{text:"未生成建议。手动填写时段也可使用。"}
                    Small{text:"AI 只辅助建议或草稿；轮询不调用模型。活动摘要去除账号、房间、名字和标题。错误输出不能改变队列或容量。"}
                    fault:=Action{text:"正式服务关闭故障注入"}
                }
    }
}
#[derive(Script,ScriptHook,Widget)]pub struct BuWeiView{
    #[deref]view:View,
    #[rust]sender:Option<SyncSender<Command>>,
    #[rust]receiver:Option<Receiver<super::Result<HostView>>>,
    #[rust]busy:bool,
    #[rust]timer:Timer,
    #[rust]timer_started:bool,
    #[rust]scope:Option<makepad_app_module::InstanceScope>,
    #[rust]last_account:Option<String>,
    #[rust]last_participant_operation:Option<String>,
    #[rust]consent_id:Option<String>,
    #[rust]tab:u8,
    #[rust]last_note:Option<String>,
}
impl BuWeiView{
    fn display(&mut self,cx:&mut Cx,view:HostView){
        if self.last_account.as_deref()!=Some(view.account.as_str()){
            let (title,markdown)=view.draft.clone().unwrap_or_default();
            self.view.text_input(cx,ids!(article_title)).set_text(cx,&title);self.view.text_input(cx,ids!(markdown)).set_text(cx,&markdown);
            self.last_participant_operation=None;self.last_note=None;
            self.last_account=Some(view.account.clone());
        }
        if let Some((id,title,markdown))=&view.generated_note{if self.last_note.as_ref()!=Some(id){
            self.view.text_input(cx,ids!(article_title)).set_text(cx,title);self.view.text_input(cx,ids!(markdown)).set_text(cx,markdown);self.last_note=Some(id.clone());
        }}
        // Apply host-validated preferences once per new preview. Polling must
        // preserve subsequent edits, which invalidate the old confirmation.
        if let Some((id,p))=&view.participant_inputs{if self.last_participant_operation.as_ref()!=Some(id){
            self.view.text_input(cx,ids!(earliest)).set_text(cx,&p.earliest.to_string());
            self.view.text_input(cx,ids!(latest)).set_text(cx,&p.latest.to_string());
            self.view.text_input(cx,ids!(group)).set_text(cx,&p.group.to_string());
            self.last_participant_operation=Some(id.clone());
        }}
        self.view.label(cx,ids!(profile)).set_text(cx,if std::env::var("BUWEI_PROFILE").as_deref()==Ok("participant"){"参与者窗口 · 请登录本人 Rinx 账号"}else{"组织者窗口 · 登录身份由 Rinx 核验"});
        self.view.label(cx,ids!(account)).set_text(cx,&format!("当前真实账号：{} · {}",view.account,if view.authorized{"已授权"}else{"未授权"}));
        self.consent_id=view.consent_id.clone();self.view.label(cx,ids!(authorization)).set_text(cx,&view.consent);self.view.label(cx,ids!(expires)).set_text(cx,&view.expires);
        self.view.label(cx,ids!(activity)).set_text(cx,&view.activity);self.view.label(cx,ids!(people)).set_text(cx,&view.people);
        self.view.label(cx,ids!(sync_status)).set_text(cx,&view.sync_status);
        self.view.label(cx,ids!(model_status)).set_text(cx,&view.model_status);
        self.view.label(cx,ids!(authorization)).set_visible(cx,view.consent_id.is_some());
        self.view.label(cx,ids!(participant_preview)).set_visible(cx,!view.organizer);
        self.view.label(cx,ids!(preview)).set_visible(cx,view.organizer);
        self.view.view(cx,ids!(participant_confirmation)).set_visible(cx,!view.organizer);
        self.view.button(cx,ids!(nav_activity)).set_text(cx,if view.organizer{"活动与候补"}else{"报名与邀请"});
        self.pages(cx);
        self.view.view(cx,ids!(organizer_actions)).set_visible(cx,view.organizer);
        self.view.view(cx,ids!(organizer_setup)).set_visible(cx,view.organizer);
        self.view.view(cx,ids!(participant_actions)).set_visible(cx,!view.organizer);
        self.view.view(cx,ids!(participant_setup)).set_visible(cx,!view.organizer);
        self.view.label(cx,ids!(participant_preview)).set_text(cx,&view.participant_preview);self.view.label(cx,ids!(history)).set_text(cx,&view.history);
        self.view.label(cx,ids!(reply_status)).set_text(cx,&view.reply);self.view.label(cx,ids!(preview)).set_text(cx,&view.preview);self.view.label(cx,ids!(article)).set_text(cx,&view.article);
        self.view.label(cx,ids!(advice)).set_text(cx,&view.advice);self.view.label(cx,ids!(message)).set_text(cx,&view.message);
        self.view.button(cx,ids!(fault)).set_text(cx,if view.fault{"丢弃本地回执：开"}else{"丢弃本地回执：关"});self.view.redraw(cx);
    }
    fn pages(&mut self,cx:&mut Cx){
        self.view.view(cx,ids!(page_activity)).set_visible(cx,self.tab==0);
        self.view.view(cx,ids!(page_confirm)).set_visible(cx,self.tab==1);
        self.view.view(cx,ids!(page_history)).set_visible(cx,self.tab==2);
        self.view.view(cx,ids!(page_article)).set_visible(cx,self.tab==3);
        self.view.view(cx,ids!(page_help)).set_visible(cx,self.tab==4);
        self.view.redraw(cx);
    }
    fn send(&mut self,cx:&mut Cx,command:Command){
        if self.busy{return;}
        if matches!(&command,Command::Prepare|Command::Join(_)|Command::Accept(_)|Command::Cancel|Command::ApplySuggestion(_)){self.tab=1;self.pages(cx);}
        if let Some(tx)=&self.sender{if tx.try_send(command).is_ok(){self.busy=true;self.view.label(cx,ids!(message)).set_text(cx,"正在处理，请等待结果。重复点击不会重复发送。");self.view.redraw(cx);}}
    }
}
impl Widget for BuWeiView{
    fn draw_walk(&mut self,cx:&mut Cx2d,scope:&mut Scope,walk:Walk)->DrawStep{self.view.draw_walk(cx,scope,walk)}
    fn handle_event(&mut self,cx:&mut Cx,event:&Event,scope:&mut Scope){
        if !self.timer_started {self.timer=cx.start_interval(0.25);self.timer_started=true;if let Some(scope)=self.scope{super::host::timer(scope,self.timer);}}
        if self.timer.is_event(event).is_some(){
            let updates=self.receiver.as_ref().map(|r|r.try_iter().collect::<Vec<_>>()).unwrap_or_default();for update in updates{self.busy=false;match update{Ok(view)=>self.display(cx,view),Err(message)=>{self.view.label(cx,ids!(message)).set_text(cx,&message);self.view.redraw(cx);}}}
        }
        if let Event::Actions(actions)=event{
            macro_rules! clicked{($id:ident)=>{self.view.button(cx,ids!($id)).clicked(actions)}}
            if let Some(tab)=if clicked!(nav_activity){Some(0)}else if clicked!(nav_confirm){Some(1)}else if clicked!(nav_history){Some(2)}else if clicked!(nav_article){Some(3)}else if clicked!(nav_help){Some(4)}else{None}{self.tab=tab;self.pages(cx);return;}
            macro_rules! text{($id:ident)=>{self.view.text_input(cx,ids!($id)).text()}}
            let parse=|value:String|value.parse::<u8>().map_err(|_|"请填写整数小时与人数");
            let command:super::Result<Option<Command>>=(||{
                Ok(if clicked!(authorize){Some(Command::Authorize)}else if clicked!(confirm_authorization){Some(Command::ConfirmAuthorization(self.consent_id.clone().ok_or("请先查看授权范围")?))}else if clicked!(switch){Some(Command::Switch)}else if clicked!(revoke){Some(Command::Revoke)}else if clicked!(refresh){Some(Command::Refresh)}
                else if clicked!(invite_member){Some(Command::InviteMember(text!(member)))}else if clicked!(sync_activity){Some(Command::SyncActivity)}else if clicked!(join_room){Some(Command::JoinRoom(text!(room)))}
                else if clicked!(create){Some(Command::Create{title:text!(title),capacity:parse(text!(capacity))?,start:parse(text!(start))?,end:parse(text!(end))?})}
                else if clicked!(join){Some(Command::Join(Preferences{earliest:parse(text!(earliest))?,latest:parse(text!(latest))?,group:parse(text!(group))?}))}
                else if clicked!(participant_confirm){Some(Command::ConfirmParticipant(Preferences{earliest:parse(text!(earliest))?,latest:parse(text!(latest))?,group:parse(text!(group))?}))}else if clicked!(pending_reconcile){Some(Command::ReconcilePending)}
                else if clicked!(prepare){Some(Command::Prepare)}else if clicked!(execute){Some(Command::Execute)}else if clicked!(reconcile){Some(Command::Reconcile)}
                else if clicked!(accept){Some(Command::Accept(true))}else if clicked!(decline){Some(Command::Accept(false))}else if clicked!(cancel){Some(Command::Cancel)}else if clicked!(expire){Some(Command::Expire)}
                else if clicked!(suggest){Some(Command::Suggest(text!(requirement)))}else if clicked!(apply_advice){Some(Command::ApplySuggestion(text!(requirement)))}
                else if clicked!(explain){Some(Command::Explain)}else if clicked!(generate_note){Some(Command::GenerateNote)}else if clicked!(apply_note){Some(Command::ApplyNote{title:text!(article_title),markdown:text!(markdown)})}else if clicked!(configure_model){Some(Command::ConfigureModel)}
                else if clicked!(draft){Some(Command::Draft{title:text!(article_title),markdown:text!(markdown)})}else if clicked!(new_article){Some(Command::NewArticle{title:text!(article_title),markdown:text!(markdown)})}else if clicked!(article_prepare){Some(Command::PrepareArticle)}else if clicked!(publish){Some(Command::PublishArticle{title:text!(article_title),markdown:text!(markdown)})}else if clicked!(article_reconcile){Some(Command::ReconcileArticle)}else if clicked!(fault){Some(Command::Fault)}else{None})
            })();match command{Ok(Some(command))=>self.send(cx,command),Err(message)=>{self.view.label(cx,ids!(message)).set_text(cx,&message);self.view.redraw(cx);},_=>{}}
        }
        self.view.handle_event(cx,event,scope);
        if matches!(event,Event::Scroll(_)){self.view.redraw(cx);}
    }
}

pub struct BuWeiModule;
pub static MODULE:BuWeiModule=BuWeiModule;
impl AppModule for BuWeiModule {
    fn id(&self)->&'static str{"buwei"}
    fn label(&self)->&'static str{"补位"}
    fn register(&self,vm:&mut ScriptVm){script_mod(vm);}
    fn open_schema(&self)->OpenSchema{OpenSchema::new(1)}
    fn capabilities(&self)->&'static [&'static str]{&["storage","model","matrix"]}
    fn create(&self,vm:&mut ScriptVm,_open:ValidatedOpen,handles:InstanceHandles)->InstanceParts{
        let scope=handles.scope;
        let (sender,receiver)=super::host::open(scope);
        let value=script_eval!(vm,{use mod.widgets.* BuWeiView{}});
        let root=WidgetRef::script_from_value(vm,value);
        vm.with_cx_mut(|cx|{if let Some(mut view)=root.borrow_mut::<BuWeiView>(){view.sender=Some(sender);view.receiver=Some(receiver);view.busy=true;view.scope=Some(scope);}root.redraw(cx);});
        let closing_root=root.clone();
        InstanceParts{root,executor:Box::new(BuWeiExecutor),shutdown:Box::new(move |vm|{super::host::close(scope);vm.with_cx_mut(|cx|{if let Some(mut view)=closing_root.borrow_mut::<BuWeiView>(){if view.timer_started{cx.stop_timer(view.timer);view.timer_started=false;}view.sender=None;view.receiver=None;}});})}
    }
}
struct BuWeiExecutor;
impl ServiceExecutor for BuWeiExecutor{
    fn manifest(&self)->ServiceManifest{ServiceManifest::new("buwei","补位","宿主授权与回执应用；发送需要本人确认。")}
    fn execute(&mut self,_cx:&mut Cx,call:&ServiceCall)->ExecOutcome{ExecOutcome::Done(ToolResult::unavailable(&call.call_id,"请在宿主界面预览并确认操作"))}
}
