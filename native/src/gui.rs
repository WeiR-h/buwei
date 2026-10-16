use super::Preferences;
use super::controller::intentions::{GoalForm, IntentCommand};
use super::controller::{Command, View as HostView};
use buwei_host_core::assistance::{GoalKind, GoalStatus, PersonalPreferences};
use buwei_host_core::proactive::AssistanceCard;
use buwei_host_core::interaction::ParticipantControls;
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
    use mod.text.*
    let BodyStyle=TextStyle{font_size:13 line_spacing:1.35 font_family:FontFamily{
        latin:=FontMember{res:crate_resource("makepad_widgets:resources/Inter.ttf") asc:0.0 desc:0.0 weight:400.0}
        chinese:=FontMember{res:crate_resource("self:resources/fonts/SourceHanSansSC-Regular.otf") asc:0.0 desc:0.0 weight:400.0}
    }}
    let StrongStyle=BodyStyle{font_family:FontFamily{
        latin:=FontMember{res:crate_resource("makepad_widgets:resources/Roboto-Bold.ttf") asc:0.0 desc:0.0 weight:700.0}
        chinese:=FontMember{res:crate_resource("self:resources/fonts/SourceHanSansSC-Bold.otf") asc:0.0 desc:0.0 weight:700.0}
    }}
    let Text=Label{width:Fill height:Fit padding:0 draw_text.color:#x193c32 draw_text.wrap:Words draw_text.text_style:BodyStyle}
    let Small=Text{draw_text.color:#x526a60 draw_text.text_style.font_size:11}
    let Heading=Text{draw_text.text_style:StrongStyle{font_size:18}}
    let Action=ButtonFlat{height:42 padding:Inset{left:16 right:16 top:8 bottom:8}
        draw_text.text_style:BodyStyle draw_text.color:#x244239 draw_text.color_hover:#x173d30 draw_text.color_focus:#x173d30
        draw_bg +: {color:#xffffff color_hover:#xf0f6f1 color_down:#xe3eee6 color_focus:#xe8f1eb color_2:vec4(-1.0,-1.0,-1.0,-1.0) color_2_hover:vec4(-1.0,-1.0,-1.0,-1.0) color_2_down:vec4(-1.0,-1.0,-1.0,-1.0) color_2_focus:vec4(-1.0,-1.0,-1.0,-1.0)
            border_size:1.0 border_radius:9.0 color_dither:0.0 border_color:#xd6e1d8 border_color_2:vec4(-1.0,-1.0,-1.0,-1.0) border_color_hover:#x91b49e border_color_focus:#x257254}
    }
    let Primary=Action{draw_text.color:#xffffff draw_text.color_hover:#xffffff draw_text.color_down:#xffffff draw_text.color_focus:#xffffff
        draw_bg +: {color:#x1c654b color_hover:#x257855 color_down:#x154f3a color_focus:#x1c654b border_color:#x1c654b border_color_2:vec4(-1.0,-1.0,-1.0,-1.0) border_color_hover:#x257855 border_color_focus:#x123d2d}}
    let Nav=Action{width:Fill height:44 align:Align{x:0.0 y:0.5} draw_bg +: {color:#x0000 color_hover:#xebf3ed color_down:#xdcece0 color_focus:#xebf3ed border_size:0.0}}
    let NavRow=View{width:Fill height:44 show_bg:true draw_bg +: {nav_tint:uniform(#x0000) pixel:fn(){return self.nav_tint;}}}
    let CardAction=Action{width:Fill height:104 align:Align{x:0.0 y:0.5} padding:18 draw_text.text_style:BodyStyle{font_size:14} draw_bg.border_radius:12.0}
    let Panel=RoundedView{width:Fill height:Fit flow:Down spacing:14 padding:24 show_bg:true draw_bg +: {color:#xffffff border_color:#xe0e8e1 border_size:1.0 border_radius:14.0}}
    let Input=TextInput{width:Fill height:44 draw_text.text_style:BodyStyle draw_text.color:#x193c32 draw_bg.color:#xffffff}
    mod.widgets.BuWeiView = set_type_default() do #(BuWeiView::register_widget(vm)) {
        ..mod.widgets.RectView
        width:Fill height:Fill flow:Right padding:0 spacing:0 show_bg:true draw_bg.color:#xf6f8f5
        View{width:188 height:Fill flow:Down padding:18 spacing:8 show_bg:true draw_bg.color:#xffffff
            View{width:Fill height:Fit flow:Right spacing:10 align:Align{x:0.0 y:0.5}
                brand_mark:=Image{width:38 height:38}
                Heading{width:Fit text:"补位" draw_text.text_style.font_size:24}
            }
            Small{text:"让想来的人，刚好有位" margin:Inset{bottom:20 top:6}}
            nav_row_activity:=NavRow{draw_bg.nav_tint:#xe7f1e9 nav_activity:=Nav{text:"我的活动"}}
            nav_row_discover:=NavRow{nav_discover:=Nav{text:"发现活动"}}
            nav_row_goals:=NavRow{nav_goals:=Nav{text:"我的意图"}}
            nav_confirm:=Nav{text:"待确认操作" visible:false}
            nav_row_history:=NavRow{nav_history:=Nav{text:"活动记录"}}
            nav_row_article:=NavRow{nav_article:=Nav{text:"活动小记"}}
            View{width:Fill height:Fill}
            nav_row_help:=NavRow{nav_help:=Nav{text:"账号与设置"}}
            Small{text:#(concat!("v",env!("CARGO_PKG_VERSION")," · Windows"))}
        }
        workspace:=View{width:Fill height:Fill flow:Down padding:Inset{left:24 right:24 top:22 bottom:96} spacing:12
            View{width:Fill height:Fit flow:Right spacing:18 align:Align{x:0.0 y:0.5}
                View{width:Fill height:Fit flow:Down spacing:6
                    workspace_title:=Heading{text:"我的活动" draw_text.text_style.font_size:26}
                    workspace_subtitle:=Small{text:"组织下一场相聚，也找到适合自己的活动。"}
                }
                new_activity:=Primary{text:"新建活动" visible:false}
            }
            View{width:Fill height:Fit flow:Right spacing:12
                account:=Small{width:Fill text:"本人登录身份由 Rinx 核验"}
                profile:=Small{width:300 text:""}
            }
            expires:=Small{visible:false}
            message:=Small{text:"请先完成本人登录，开启活动工作台。"}
                page_activity:=ScrollYView{width:Fill height:Fill flow:Down spacing:12 padding:16 show_bg:true draw_bg.color:#xf2f6ef
                    activity_heading:=Heading{text:"我的活动" visible:false}
                    welcome:=Panel{visible:false
                        welcome_title:=Heading{text:"欢迎来到补位" draw_text.text_style.font_size:22}
                        welcome_copy:=Text{text:"先在 Rinx 完成本人登录，再创建或挑选活动。"}
                        welcome_settings:=Primary{text:"查看登录与授权"}
                        welcome_discover:=Action{text:"发现可以参加的活动" visible:false}
                    }
                    assistance_area:=View{width:Fill height:Fit flow:Down spacing:8 visible:false
                        Heading{text:"补位发现了这些变化"}
                        assistance_0:=CardAction{height:136 visible:false}
                        assistance_1:=CardAction{height:136 visible:false}
                        assistance_2:=CardAction{height:136 visible:false}
                        View{width:Fill height:Fit flow:Right spacing:8
                            assistance_follow:=Action{text:"继续跟进本页首条候补" visible:false}
                            assistance_prev:=Action{text:"上一条"} assistance_next:=Action{text:"下一条"}
                            assistance_snooze:=Action{text:"本页首条稍后提醒"} assistance_ignore:=Action{text:"忽略首条"} assistance_disable:=Action{text:"不再推荐这类"}
                        }
                    }
                    card_status:=Text{}
                    join_card:=Action{text:"确认加入并查看活动" visible:false}
                    home_current:=Text{}
                    home_status:=Text{}
                    reply_status:=Text{text:"当前没有邀请。"}
                    participant_actions:=View{width:Fill height:Fit flow:Right spacing:8
                        accept:=Action{text:"预览本人接受"} decline:=Action{text:"预览本人拒绝"} cancel:=Action{text:"预览本人取消"}
                    }
                    View{width:Fill height:Fit flow:Right spacing:8
                        open_current:=Action{text:"查看当前活动"}
                        back_to_activities:=Action{text:"返回活动列表" visible:false}
                    }
                    organizer_new_actions:=View{width:Fill height:Fit flow:Right spacing:8
                        copy_activity:=Action{text:"复制下一场"} archive_activity:=Action{text:"归档本场"}
                    }
                    activity_cards:=View{width:Fill height:Fit flow:Down spacing:12
                    activity_0:=CardAction{visible:false}
                    activity_1:=CardAction{visible:false}
                    activity_2:=CardAction{visible:false}
                    activity_3:=CardAction{visible:false}
                    activity_4:=CardAction{visible:false}
                    activity_pagination:=View{width:Fill height:Fit flow:Right spacing:8 activity_prev:=Action{text:"上一页"} activity_next:=Action{text:"下一页"}}
                    }
                    campus_entry:=Panel{visible:false padding:14 spacing:8
                            Heading{text:"校园活动 · 从合适的场景开始"}
                            Small{text:"策划、发布、报名与候补连在一起；支持公开活动和指定社群。"}
                            View{width:Fill height:Fit flow:Right spacing:6
                                campus_lecture:=Action{width:Fill text:"讲座"} campus_competition:=Action{width:Fill text:"竞赛"}
                                campus_movie:=Action{width:Fill text:"观影"} campus_festival:=Action{width:Fill text:"音乐节"} campus_club:=Action{width:Fill text:"社团招新"}
                            }
                        }
                    assistant_area:=Panel{visible:false
                        assistant_heading:=Heading{text:"先说说你的想法"}
                        requirement_caption:=Small{text:"描述活动需求或本次安排，AI 会准备可编辑的建议。"}
                        requirement_field:=View{width:Fill height:Fit requirement:=Input{height:70 is_multiline:true empty_text:"例如：下周六晚上七点，办一场六人的读书会"}}
                        clarification_field:=View{width:Fill height:Fit visible:false clarification:=Input{height:55 is_multiline:true empty_text:"只补充本次变化，未提及的信息保持原值"}}
                        View{width:Fill height:Fit flow:Right spacing:8
                            create_with_ai:=Primary{text:"准备可编辑策划稿"}
                            suggest:=Primary{text:"理解这次安排"} clarify:=Action{text:"合并补充"} apply_advice:=Primary{text:"核对并预览报名"}
                            explain:=Action{text:"为什么这样候补？"}
                        }
                        advice:=Text{}
                        ai_result:=Text{}
                        question_field:=View{width:Fill height:Fit question:=Input{empty_text:"问问这场活动：几点开始、还有多少名额？"}}
                        ask:=Action{text:"询问本场活动"}
                    }
                    activity_details:=View{width:Fill height:Fit flow:Down spacing:12 visible:false
                    seat_overview:=View{width:Fill height:Fit flow:Right spacing:12
                        Panel{width:Fill padding:18 Small{text:"已确认"} confirmed_seats:=Heading{text:"0" draw_text.text_style.font_size:24}}
                        Panel{width:Fill padding:18 Small{text:"为候补保留"} reserved_seats:=Heading{text:"0" draw_text.text_style.font_size:24 draw_text.color:#x8a5f20}}
                        Panel{width:Fill padding:18 Small{text:"剩余名额"} free_seats:=Heading{text:"0" draw_text.text_style.font_size:24 draw_text.color:#x1c654b}}
                    }
                    activity:=Text{text:"尚无活动"}
                    people:=Text{text:"尚无候补"}
                    assistant_toggle:=Action{text:"用自然语言调整或询问本场活动"}
                    roster_export:=View{width:Fill height:Fit flow:Down spacing:8
                        View{width:Fill height:Fit flow:Right spacing:8 roster_prepare:=Action{text:"导出报名名单…"} roster_confirm:=Action{text:"确认保存名单" visible:false}}
                        roster_preview:=Text{}
                    }
                    organizer_invite_entry:=View{width:Fill height:Fit flow:Right
                        prepare:=Action{text:"预览手动邀请"}
                    }
                    organizer_setup:=View{width:Fill height:Fit flow:Down spacing:8
                        creation_form:=View{width:Fill height:Fit flow:Down spacing:8
                        Heading{text:"活动策划草稿"}
                        View{width:Fill height:Fit flow:Right spacing:8
                            template_badminton:=Action{text:"羽毛球"} template_boardgame:=Action{text:"桌游"} template_reading:=Action{text:"读书会"}
                            templates_toggle:=Action{text:"更多活动类型…"}
                        }
                        template_current:=Small{text:"当前类型：自定义活动"}
                        template_library:=Panel{visible:false
                            Heading{text:"选一个合适的活动起点"}
                            Small{text:"校园活动 · 运动健身 · 户外出行 · 文娱休闲 · 学习交流 · 聚会生活 · 社群共建"}
                            View{width:Fill height:Fit flow:Right spacing:8
                                template_search:=Input{width:Fill empty_text:"搜索活动或分类，例如：跑步、户外、聚餐"}
                                template_search_apply:=Action{text:"搜索"} template_search_clear:=Action{text:"全部"}
                            }
                            View{width:Fill height:Fit flow:Right spacing:8
                                template_option_0:=Action{width:Fill height:66 align:Align{x:0.0 y:0.5}} template_option_1:=Action{width:Fill height:66 align:Align{x:0.0 y:0.5}} template_option_2:=Action{width:Fill height:66 align:Align{x:0.0 y:0.5}}
                            }
                            View{width:Fill height:Fit flow:Right spacing:8
                                template_option_3:=Action{width:Fill height:66 align:Align{x:0.0 y:0.5}} template_option_4:=Action{width:Fill height:66 align:Align{x:0.0 y:0.5}} template_option_5:=Action{width:Fill height:66 align:Align{x:0.0 y:0.5}}
                            }
                            template_results:=Small{}
                            template_pagination:=View{width:Fill height:Fit flow:Right spacing:8
                                template_prev:=Action{text:"上一页"} template_next:=Action{text:"下一页"}
                            }
                            Small{text:"选择模板会保存一份新策划稿，原草稿保留。人数与流程均可调整；日期、地点需你确认。"}
                        }
                        Small{text:"活动名称"} title:=Input{text:"社群活动" empty_text:"给这场活动起一个名字"}
                        Small{text:"地点"} location:=Input{empty_text:"地点未定时，先在策划稿中说明"}
                        Small{text:"活动说明"} description:=Input{height:70 is_multiline:true empty_text:"活动目标、内容与参加须知"}
                        View{width:Fill height:Fit flow:Right spacing:8
                            Small{text:"名额" width:40} capacity:=Input{text:"6" width:60}
                            Small{text:"开始" width:40} start:=Input{width:Fill empty_text:"YYYY-MM-DD HH:mm"}
                            Small{text:"结束" width:40} end:=Input{width:Fill empty_text:"YYYY-MM-DD HH:mm"}
                        }
                        campus_toggle:=Action{visible:false text:"添加校园观影信息"}
                        campus_form:=Panel{visible:false padding:16 spacing:8
                            Heading{text:"校园活动信息"}
                            Small{text:"以下内容经确认后随活动公开；无需填写学号、手机号或私人资料。"}
                            View{width:Fill height:Fit flow:Right spacing:10
                                View{width:Fill height:Fit flow:Down Small{text:"学校 / 校区（选填）"} campus_school:=Input{empty_text:"例如：某大学东校区"}}
                                View{width:Fill height:Fit flow:Down Small{text:"面向人群（选填）"} campus_audience:=Input{empty_text:"例如：对主题感兴趣的同学"}}
                            }
                            View{width:Fill height:Fit flow:Right spacing:10
                                View{width:Fill height:Fit flow:Down campus_topic_label:=Small{text:"主题"} campus_topic:=Input{empty_text:"本人确认的主题 / 作品 / 场次"}}
                                View{width:Fill height:Fit flow:Down campus_guests_label:=Small{text:"主讲 / 组织团队（选填）"} campus_guests:=Input{empty_text:"仅填写已确认的安排"}}
                            }
                            campus_requirements_label:=Small{text:"参加须知（选填）"}
                            campus_requirements:=Input{height:65 is_multiline:true empty_text:"活动内容、入场、准备与参与角色"}
                            campus_role_row:=View{width:Fill height:Fit flow:Right spacing:8 visible:false
                                Small{text:"音乐节本场用途" width:110} campus_role:=Input{width:170 empty_text:"请选择本场用途"}
                                campus_role_audience:=Action{text:"观众报名"} campus_role_performer:=Action{text:"演出招募"}
                            }
                            View{width:Fill height:Fit flow:Right spacing:10
                                View{width:Fill height:Fit flow:Down Small{text:"报名截止 · 北京时间（选填）"} campus_deadline:=Input{empty_text:"YYYY-MM-DD HH:mm；留空则到活动开始"}}
                                campus_team_row:=View{width:200 height:Fit flow:Down visible:false Small{text:"竞赛每份报名人数（必填）"} campus_team:=Input{empty_text:"个人赛 1；团队赛 2–8"}}
                            }
                            campus_checklist:=Small{}
                            Small{text:"截止后停止新增及修改报名；已有候补仍可递补，已送达邀请可在有效期内回复。每场最多 30 个名额，按人数计入。"}
                        }
                        create:=Primary{text:"核对并预览创建"}
                        plan_0:=Action{width:Fill visible:false} plan_1:=Action{width:Fill visible:false} plan_2:=Action{width:Fill visible:false}
                        plan_summary:=Text{}
                        View{width:Fill height:Fit flow:Right spacing:8
                            plan_revise:=Action{text:"合并本次补充"} plan_preview_create:=Action{text:"重新预览策划"} plan_confirm_create:=Primary{text:"确认创建本场活动" visible:false}
                        }
                        plan_creation_preview:=Text{}
                        recover_setup:=Action{text:"恢复待核实的活动创建"}
                        cancel_setup:=Action{text:"收起活动草稿"}
                        }
                        sharing_setup:=View{width:Fill height:Fit flow:Down spacing:10
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
                        }
                        automation_setup:=View{width:Fill height:Fit flow:Down spacing:10
                        Heading{text:"自动补位"}
                        automation_status:=Text{}
                        View{width:Fill height:Fit flow:Right spacing:8
                            Small{text:"邀请分钟" width:70} invitation_minutes:=Input{text:"10" width:60}
                            Small{text:"发送时段" width:70} quiet_start:=Input{text:"8" width:60} Small{text:"至" width:20} quiet_end:=Input{text:"22" width:60}
                            Small{text:"本次上限" width:70} max_invitations:=Input{text:"30" width:60}
                        }
                        View{width:Fill height:Fit flow:Right spacing:8
                            preview_automation:=Action{text:"查看自动补位规则"} confirm_automation:=Action{text:"确认启用"} pause_automation:=Action{text:"暂停自动补位"}
                        }
                        policy_preview:=Text{}
                        }
                    }
                    participant_setup:=View{width:Fill height:Fit flow:Down spacing:8
                        quick_join:=Primary{text:"参加整场 · 准备报名"}
                        Small{text:"从 Rinx 聊天打开活动卡片，即可用本人账号报名。"}
                        legacy_room_setup:=View{visible:false width:Fill height:Fit flow:Down
                            room:=Input{empty_text:"高级恢复：完整房间编号 !..."}
                            join_room:=Action{text:"本人加入活动房间"}
                        }
                        Small{text:"先核对本场活动日期和本人可用时段，再预览报名。请输入 YYYY-MM-DD HH:mm 格式的完整日期与时间。"}
                        View{width:Fill height:Fit flow:Right spacing:8
                            earliest:=Input{width:Fill empty_text:"最早可用日期和时间"}
                            latest:=Input{width:Fill empty_text:"最晚可用日期和时间"}
                            group:=Input{text:"1" width:90 empty_text:"同行人数"}
                            join:=Primary{text:"预览本人报名"}
                        }
                    }
                    }
                }
                page_discover:=ScrollYView{visible:false width:Fill height:Fill flow:Down spacing:12 padding:16 show_bg:true draw_bg.color:#xf2f6ef
                    Heading{text:"发现活动 · 公开与我的社群"}
                    View{width:Fill height:Fit flow:Right spacing:8 public_open:=Action{text:"公开活动大厅"} community_mode:=Action{text:"我的社群"} public_confirm:=Action{text:"确认进入公开大厅" visible:false}}
                    public_preview:=Text{}
                    View{width:Fill height:Fit flow:Right spacing:8
                        communities_load:=Action{text:"选择已加入的 Rinx 社群"} communities_refresh:=Action{text:"更新社群活动"}
                    }
                    View{width:Fill height:Fit flow:Right spacing:8
                        community_search:=Input{width:Fill empty_text:"按名称搜索已加入的校园或兴趣社群"}
                        community_search_clear:=Action{text:"全部社群"}
                    }
                    community_results:=Small{}
                    community_0:=Action{width:Fill visible:false} community_1:=Action{width:Fill visible:false} community_2:=Action{width:Fill visible:false}
                    community_pagination:=View{width:Fill height:Fit flow:Right spacing:8 visible:false community_prev:=Action{text:"上一页社群"} community_next:=Action{text:"下一页社群"}}
                    community_selected:=Small{text:"范围由你选择；仅读取选定社群的活动卡片与加入申请。"}
                    community_status:=Small{}
                    View{width:Fill height:Fit flow:Right spacing:8
                        filter_template:=Input{width:180 empty_text:"校园 / 讲座 / 竞赛 / 不限"} filter_date:=Input{width:160 empty_text:"日期（可留空）"}
                        filter_apply:=Action{text:"筛选"} filter_available:=Action{text:"只看发布时有空位"}
                        filter_campus:=Action{text:"校园活动"}
                    }
                    listing_0:=CardAction{height:155 visible:false} listing_1:=CardAction{height:155 visible:false} listing_2:=CardAction{height:155 visible:false}
                    listing_pagination:=View{width:Fill height:Fit flow:Right spacing:8 visible:false listing_prev:=Action{text:"上一页活动"} listing_next:=Action{text:"下一页活动"}}
                    community_organizer_actions:=View{width:Fill height:Fit flow:Down spacing:8
                        Heading{text:"发布当前活动到社群"}
                        View{width:Fill height:Fit flow:Right spacing:8 community_publish:=Action{text:"预览社群活动卡片"} community_withdraw:=Action{text:"预览收起本场卡片"} community_confirm_publish:=Action{text:"确认社群发布" visible:false}}
                        View{width:Fill height:Fit flow:Right spacing:8 access_prepare:=Action{text:"查看社群加入规则"} access_confirm:=Action{text:"确认启用加入规则" visible:false} access_pause:=Action{text:"暂停社群自动邀请"}}
                        access_preview:=Text{} access_status:=Text{}
                    }
                    community_preview:=Text{}
                    View{width:Fill height:Fit flow:Right spacing:8
                        entry_confirm:=Action{text:"确认本人加入申请" visible:false} discovery_join:=Action{text:"加入并准备报名" visible:false} discovery_current:=Action{text:"打开本场并准备报名"} community_reconcile:=Action{text:"核实社群操作回执"}
                    }
                    entry_progress:=Text{}
                    Small{text:"卡片显示发布时的活动信息；加入后核验本场的最新状态。加入申请不占位，报名与接受均由本人确认。"}
                }
                page_goals:=ScrollYView{visible:false width:Fill height:Fill flow:Down spacing:12 padding:16 show_bg:true draw_bg.color:#xf2f6ef
                    Heading{text:"我的目标 · 让补位持续帮你跟进"}
                    Small{text:"目标和长期偏好按本人账号保存在本机。填写的本次要求优先；活动中的其他成员看不到这些个人资料。"}
                    View{width:Fill height:Fit flow:Right spacing:8
                        new_organize_goal:=Action{text:"组织这场活动"} new_participate_goal:=Action{text:"找到适合我的活动"}
                    }
                    goal_request:=Input{height:65 is_multiline:true empty_text:"例如：周末晚上想打羽毛球，通常两个人。这次有空的具体日期和时段可以一起填写。"}
                    View{width:Fill height:Fit flow:Right spacing:8
                        goal_ai_organize:=Action{text:"AI 准备组织目标"} goal_ai_participate:=Action{text:"AI 理解参与意愿"}
                    }
                    goal_0:=Action{width:Fill height:90 visible:false} goal_1:=Action{width:Fill height:90 visible:false} goal_2:=Action{width:Fill height:90 visible:false}
                    goal_pagination:=View{width:Fill height:Fit flow:Right spacing:8 visible:false goal_prev:=Action{text:"上一页目标"} goal_next:=Action{text:"下一页目标"}}
                    intention_result:=Text{}
                    feedback_status:=Text{}
                    undo_feedback:=Action{text:"撤回最近反馈" visible:false}
                    goal_editor:=View{width:Fill height:Fit flow:Down spacing:8 visible:false
                        Heading{text:"核对并保存目标"}
                        goal_sources:=Small{}
                        goal_title:=Input{empty_text:"想完成什么"}
                        goal_template:=Input{empty_text:"活动类型，如：跑步、徒步、聚餐，或不限类型"}
                        goal_schedule:=View{width:Fill height:Fit flow:Right spacing:8
                            goal_weekly:=Action{text:"使用我确认的每周安排"} goal_specific:=Action{text:"只按本次具体时段"}
                        }
                        goal_schedule_status:=Small{}
                        View{width:Fill height:Fit flow:Right spacing:8
                            goal_earliest:=Input{width:210 empty_text:"开始日期时间"} goal_latest:=Input{width:210 empty_text:"结束日期时间"}
                            Small{text:"同行人数" width:70} goal_group:=Input{width:65 empty_text:"1–8"}
                        }
                        goal_organizer_fields:=View{width:Fill height:Fit flow:Down spacing:8
                            View{width:Fill height:Fit flow:Right spacing:8
                                Small{text:"目标确认人数" width:100} goal_target:=Input{width:65}
                                Small{text:"检查时间" width:70} goal_check:=Input{width:210 empty_text:"开始前的完整日期时间"}
                            }
                            View{width:Fill height:Fit flow:Right spacing:8
                                Small{text:"重复周期（天）" width:100} goal_recurrence:=Input{width:65 empty_text:"留空不重复"}
                                Small{text:"提前筹备（小时）" width:120} goal_preparation:=Input{width:65 text:"24"}
                            }
                        }
                        feedback_actions:=View{width:Fill height:Fit flow:Right spacing:8
                            feedback_once:=Action{text:"类型与人数：仅这次"} feedback_long:=Action{text:"类型与人数：以后也是"}
                        }
                        correction_editor:=View{width:Fill height:Fit flow:Down spacing:8 visible:false
                            Heading{text:"更正这次安排"}
                            correction_request:=Input{height:65 is_multiline:true empty_text:"例如：这次能待到晚上九点，还是我一个人。未提及的字段保持原值。"}
                            correction_prepare:=Action{text:"AI 准备更正与差异"}
                            correction_diff:=Text{}
                            correction_confirmation:=View{width:Fill height:Fit flow:Right spacing:8 visible:false
                                correction_once:=Action{text:"确认更正：仅本次"} correction_long:=Action{text:"确认更正：更新长期偏好"}
                            }
                            registration_update:=Action{text:"另行预览本场候补更新"}
                            registration_progress:=Text{}
                        }
                        Small{text:"来源：本人填写或已确认的偏好。缺失信息请补充，保存目标不会发送消息或替你报名。"}
                        View{width:Fill height:Fit flow:Right spacing:8
                            save_goal:=Action{text:"确认保存目标"} pause_goal:=Action{text:"暂停目标"} resume_goal:=Action{text:"继续关注"} delete_goal:=Action{text:"删除目标"}
                        }
                    }
                }
                page_confirm:=ScrollYView{visible:false width:Fill height:240 flow:Down spacing:12 padding:16 show_bg:true draw_bg.color:#xfffbf3
                    Heading{text:"核对并确认本次操作"}
                    Small{text:"核对本人账号、活动、时间与人数后确认。修改内容后请重新预览。"}
                    organizer_actions:=View{width:Fill height:Fit flow:Right spacing:8
                        sync_activity:=Action{text:"同步活动与回复"}
                        execute:=Action{text:"确认邀请"}
                        reconcile:=Action{text:"核实邀请回执"} expire:=Action{text:"检查过期邀请"}
                    }
                    preview:=Text{text:"尚未预览邀请。"}
                    participant_preview:=Text{text:"本人操作尚未预览。"}
                    participant_confirmation:=View{width:Fill height:Fit flow:Right
                        participant_confirm:=Action{text:"确认本人预览操作"}
                    }
                }
                page_history:=ScrollYView{visible:false width:Fill height:Fill flow:Down spacing:12 padding:16 show_bg:true draw_bg.color:#xf2f6ef
                    Heading{text:"主动任务的进展"}
                    task_0:=Action{width:Fill height:65 visible:false}
                    task_1:=Action{width:Fill height:65 visible:false}
                    task_2:=Action{width:Fill height:65 visible:false}
                    task_pagination:=View{width:Fill height:Fit flow:Right spacing:8 visible:false
                        task_prev:=Action{text:"上一页任务"} task_next:=Action{text:"下一页任务"}
                    }
                    pause_task:=Action{text:"暂停本页首项任务" visible:false}
                    assistance_tasks:=Text{}
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
                    Heading{text:"账号与授权"}
                    View{width:Fill height:Fit flow:Right spacing:10
                        switch:=Action{text:"账号切换说明"}
                        authorize:=Action{text:"查看授权范围"}
                        confirm_authorization:=Action{text:"确认授权 1 小时"}
                        revoke:=Action{text:"撤销授权"}
                        refresh:=Action{text:"刷新记录"}
                    }
                    authorization:=Text{text:"查看本人账号的权限范围，核对后确认授权。"}
                    Heading{text:"Windows 值守"}
                    background_status:=Text{}
                    Small{text:"开启后关闭宿主窗口将隐藏到托盘，继续在已授权范围内同步和自动补位。最长一小时，到期停止。退出补位将停止；电脑关机时不能继续。"}
                    View{width:Fill height:Fit flow:Right spacing:8
                        enable_background:=Action{text:"开启托盘值守"} pause_background:=Action{text:"暂停值守"} disable_background:=Action{text:"关闭托盘值守"}
                    }
                    sync_status:=Small{text:"自动同步尚未授权"}
                    Heading{text:"本人确认的长期偏好"}
                    pref_template:=Input{empty_text:"偏好类型，如：跑步、摄影、公益志愿，或不限类型"}
                    View{width:Fill height:Fit flow:Right spacing:8
                        Small{text:"同行人数" width:75} pref_group:=Input{width:65 empty_text:"可留空"}
                        Small{text:"星期" width:40} pref_weekdays:=Input{width:180 empty_text:"1=周一，例如 6,7"}
                    }
                    View{width:Fill height:Fit flow:Right spacing:8
                        Small{text:"可用时段" width:75} pref_start:=Input{width:100 empty_text:"19:00"} pref_end:=Input{width:100 empty_text:"21:30"}
                        Small{text:"安静时段" width:75} pref_quiet_start:=Input{width:60 text:"22"} pref_quiet_end:=Input{width:60 text:"8"}
                    }
                    View{width:Fill height:Fit flow:Right spacing:8
                        save_preferences:=Action{text:"确认保存长期偏好"} reset_preferences:=Action{text:"删除偏好并恢复默认"}
                    }
                    Heading{text:"开始使用"}
                    Text{text:"1. 在 Rinx 完成本人登录，查看权限并确认授权。\n2. 组织者新建活动，选择联系人分享活动卡片。\n3. 成员从卡片进入活动，核对时段和同行人数后报名。\n4. 组织者核对规则并启用自动补位，成员亲自回复邀请。\n5. 不明结果在活动记录中沿原编号恢复。同场仅使用一个组织者宿主。"}
                    Heading{text:"模型设置"}
                    Small{text:"AI 建议会将填写的需求发送到 Kimi Code；邀请和文章按完整预览单独确认。"}
                    model_status:=Text{text:"模型配置由宿主管理"}
                    analysis_status:=Text{}
                    View{width:Fill height:Fit flow:Right spacing:8
                        preview_analysis:=Action{text:"查看主动 AI 分析说明" visible:false} confirm_analysis:=Action{text:"确认开启主动分析" visible:false} revoke_analysis:=Action{text:"撤销主动分析" visible:false}
                    }
                    configure_model:=Action{text:"填写或更新本机 Kimi Code 密钥"}
                    Small{text:"活动助手可以理解日期和报名意愿。必要信息会先显示为可编辑草稿，由本人核对确认。"}
                    fault:=Action{text:"高级诊断" visible:false}
                    Heading{text:"运行测量"}
                    Small{text:"仅保留当前账号会话各功能最近 128 次处理耗时，包括网络等待；不记录需求原文、账号、房间或密钥。这里的耗时不代表画面帧率。"}
                    performance_summary:=Small{text:"本次会话还没有测量记录。"}
                }
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
    campus_enabled: bool,
    #[rust]
    template_library_open: bool,
    #[rust]
    template_page: usize,
    #[rust]
    template_query: String,
    #[rust]
    policy_consent_id: Option<String>,
    #[rust]
    contacts: Vec<String>,
    #[rust]
    confirmation_ready: bool,
    #[rust]
    creating: bool,
    #[rust]
    pending_creation_plan: Option<String>,
    #[rust]
    signed_in: bool,
    #[rust]
    authorized: bool,
    #[rust]
    has_activity: bool,
    #[rust]
    participant_controls: ParticipantControls,
    #[rust]
    participant_join_preview: bool,
    #[rust]
    brand_loaded: bool,
    #[rust]
    assistant_expanded: bool,
    #[rust]
    plan_id: Option<String>,
    #[rust]
    plan_revision: Option<u64>,
    #[rust]
    plan_creation_nonce: Option<String>,
    #[rust]
    plan_choices: Vec<(String, String)>,
    #[rust]
    roster_nonce: Option<String>,
    #[rust]
    communities: Vec<(String, String)>,
    #[rust]
    community_page: usize,
    #[rust]
    community_source: Vec<(String,String)>,
    #[rust]
    community_query: String,
    #[rust]
    community_actor: String,
    #[rust]
    selected_community: Option<String>,
    #[rust]
    public_mode: bool,
    #[rust]
    public_nonce: Option<String>,
    #[rust]
    current_organizer: bool,
    #[rust]
    listing_keys: Vec<String>,
    #[rust]
    listing_page: usize,
    #[rust]
    community_operation: Option<(String, String)>,
    #[rust]
    access_nonce: Option<String>,
    #[rust]
    available_only: bool,
    #[rust]
    pending_policy_pause: Option<String>,
    #[rust]
    pending_revoke: bool,
    #[rust]
    current_activity: String,
    #[rust]
    goals: Vec<(String, String)>,
    #[rust]
    goal_page: usize,
    #[rust]
    goal_id: Option<String>,
    #[rust]
    goal_form: Option<GoalForm>,
    #[rust]
    correction_id: Option<String>,
    #[rust]
    last_goal_form: Option<String>,
    #[rust]
    preferences: Option<PersonalPreferences>,
    #[rust]
    last_preferences: Option<String>,
    #[rust]
    assistance_cards: Vec<AssistanceCard>,
    #[rust]
    assistance_page: usize,
    #[rust]
    feedback_id: Option<String>,
    #[rust]
    analysis_consent_id: Option<String>,
    #[rust]
    task_choices: Vec<(String, String)>,
    #[rust]
    task_page: usize,
}
impl BuWeiView {
    fn display_communities(&mut self, cx: &mut Cx) {
        self.communities=buwei_host_core::discovery::community_choices(&self.community_source,&self.community_query);
        self.community_page=self.community_page.min(self.communities.len().saturating_sub(1)/3);
        macro_rules! choice { ($id:ident,$n:expr)=>{
            let item=self.communities.get(self.community_page*3+$n);
            self.view.button(cx,ids!($id)).set_visible(cx,item.is_some());
            if let Some((_,name))=item { self.view.button(cx,ids!($id)).set_text(cx,name); }
        }; }
        choice!(community_0,0);choice!(community_1,1);choice!(community_2,2);
        self.view.view(cx,ids!(community_pagination)).set_visible(cx,self.communities.len()>3);
        let status=if self.community_source.is_empty() { "请先选择已加入的 Rinx 社群。".into() }
            else if self.communities.is_empty() { "没有匹配的已加入社群，可修改搜索或查看全部。".into() }
            else {format!("{} 个已加入社群 · 第 {} / {} 页",self.communities.len(),self.community_page+1,self.communities.len().div_ceil(3))};
        self.view.label(cx,ids!(community_results)).set_text(cx,&status);
    }
    fn campus_input(&mut self, cx: &mut Cx) -> super::Result<Option<buwei_host_core::campus::CampusDetails>> {
        if !self.campus_enabled { return Ok(None); }
        let team = self.view.text_input(cx, ids!(campus_team)).text();
        Ok(Some(buwei_host_core::campus::CampusDetails {
            campus: self.view.text_input(cx, ids!(campus_school)).text(),
            audience: self.view.text_input(cx, ids!(campus_audience)).text(),
            topic: self.view.text_input(cx, ids!(campus_topic)).text(),
            guests: self.view.text_input(cx, ids!(campus_guests)).text(),
            requirements: self.view.text_input(cx, ids!(campus_requirements)).text(),
            signup_deadline: self.view.text_input(cx, ids!(campus_deadline)).text(),
            team_size: if self.template != "competition" || team.trim().is_empty() { None }
                else { Some(team.trim().parse().map_err(|_| "竞赛每份报名人数须为 1–8 的整数")?) },
            role: if self.template == "music_festival" {
                let role = self.view.text_input(cx, ids!(campus_role)).text();
                if role.trim().is_empty() { None } else { Some(buwei_host_core::campus::EntryRole::parse(&role)?) }
            } else { None },
        }))
    }
    fn display_campus(&mut self, cx: &mut Cx) {
        self.view.view(cx, ids!(campus_form)).set_visible(cx, self.campus_enabled);
        self.view.button(cx, ids!(campus_toggle)).set_visible(cx, self.template == "movie");
        self.view.button(cx, ids!(campus_toggle)).set_text(cx, if self.campus_enabled { "收起并移除校园观影信息" } else { "添加校园观影信息" });
        self.view.view(cx, ids!(campus_team_row)).set_visible(cx, self.campus_enabled && self.template == "competition");
        self.view.view(cx, ids!(campus_role_row)).set_visible(cx, self.campus_enabled && self.template == "music_festival");
        let p = buwei_host_core::campus::playbook(&self.template);
        self.view.label(cx, ids!(campus_topic_label)).set_text(cx, p.topic);
        self.view.label(cx, ids!(campus_guests_label)).set_text(cx, p.guests);
        self.view.label(cx, ids!(campus_checklist)).set_text(cx, p.checklist);
        self.view.label(cx, ids!(campus_requirements_label)).set_text(cx, p.requirements);
    }
    fn display_templates(&mut self, cx: &mut Cx) {
        let choices = buwei_host_core::activity_types::search(&self.template_query);
        self.template_page = self.template_page.min(choices.len().saturating_sub(1) / 6);
        self.view.view(cx, ids!(template_library)).set_visible(cx, self.template_library_open);
        self.view.button(cx, ids!(templates_toggle)).set_text(cx,
            if self.template_library_open { "收起活动库" } else { "更多活动类型…" });
        self.view.label(cx, ids!(template_current)).set_text(cx,
            &format!("当前类型：{}", buwei_host_core::activity_types::name(
                if self.template.is_empty() { "custom" } else { &self.template })));
        macro_rules! template_button {
            ($id:ident, $n:expr) => {
                let item = choices.get(self.template_page * 6 + $n);
                self.view.button(cx, ids!($id)).set_visible(cx, item.is_some());
                if let Some(t) = item {
                    self.view.button(cx, ids!($id)).set_text(cx,
                        &format!("{} · {}\n建议 {} 人", t.name, t.category, t.capacity));
                }
            }
        }
        template_button!(template_option_0, 0); template_button!(template_option_1, 1);
        template_button!(template_option_2, 2); template_button!(template_option_3, 3);
        template_button!(template_option_4, 4); template_button!(template_option_5, 5);
        self.view.view(cx, ids!(template_pagination)).set_visible(cx, choices.len() > 6);
        self.view.label(cx, ids!(template_results)).set_text(cx, &if choices.is_empty() {
            "没有找到对应类型。可点“全部”选择自定义活动。".into()
        } else {
            format!("{} 个类型 · 第 {} / {} 页", choices.len(), self.template_page + 1, choices.len().div_ceil(6))
        });
    }
    fn display(&mut self, cx: &mut Cx, view: HostView) {
        self.signed_in = view.can_create;
        self.authorized = view.authorized;
        self.has_activity = !view.activity_identity.is_empty();
        self.participant_controls = view.participant_controls;
        self.participant_join_preview = view.participant_inputs.is_some();
        if !self.brand_loaded {
            self.brand_loaded = self.view.image(cx, ids!(brand_mark))
                .load_png_from_data(cx, include_bytes!("../../assets/icon.png")).is_ok();
        }
        self.view.label(cx, ids!(confirmed_seats)).set_text(cx, &format!("{} / {}",view.seat_counts[1],view.seat_counts[0]));
        self.view.label(cx, ids!(reserved_seats)).set_text(cx, &view.seat_counts[2].to_string());
        self.view.label(cx, ids!(free_seats)).set_text(cx, &view.seat_counts[3].to_string());
        self.public_mode=view.public_mode;
        self.current_organizer=view.organizer;
        self.public_nonce=view.public_directory_nonce.clone();
        self.view.button(cx,ids!(public_confirm)).set_visible(cx,self.public_nonce.is_some());
        self.view.label(cx,ids!(public_preview)).set_text(cx,&view.public_directory_preview);
        if view.public_mode {self.selected_community=view.public_room.clone();self.view.label(cx,ids!(community_selected)).set_text(cx,"当前范围：公开活动大厅 · 所有补位用户可进入查看与申请");}
        self.view.button(cx,ids!(community_publish)).set_text(cx,if view.public_mode{"预览发布为公开活动"}else{"预览发布到指定社群"});
        if self.community_actor != view.account {
            self.community_actor=view.account.clone();
            self.community_query.clear();self.community_page=0;self.selected_community=None;
            self.view.text_input(cx,ids!(community_search)).set_text(cx,"");
        }
        self.community_source=view.communities.clone();
        self.display_communities(cx);
        self.listing_keys = view.community_listings.iter().map(|l| l.key()).collect();
        self.listing_page = self
            .listing_page
            .min(self.listing_keys.len().saturating_sub(1) / 3);
        macro_rules! listing_button {($id:ident,$n:expr)=>{let item=view.community_listings.get(self.listing_page*3+$n);self.view.button(cx,ids!($id)).set_visible(cx,item.is_some());if let Some(l)=item{self.view.button(cx,ids!($id)).set_text(cx,&l.announcement.listing_text());}};}
        listing_button!(listing_0, 0);
        listing_button!(listing_1, 1);
        listing_button!(listing_2, 2);
        self.view
            .view(cx, ids!(listing_pagination))
            .set_visible(cx, self.listing_keys.len() > 3);
        self.view
            .view(cx, ids!(community_organizer_actions))
            .set_visible(cx, view.organizer);
        self.view
            .label(cx, ids!(community_status))
            .set_text(cx, &view.community_status);
        self.view
            .label(cx, ids!(community_preview))
            .set_text(cx, &view.community_preview);
        self.view
            .label(cx, ids!(access_preview))
            .set_text(cx, &view.access_preview);
        self.view
            .label(cx, ids!(access_status))
            .set_text(cx, &view.access_status);
        self.view
            .label(cx, ids!(entry_progress))
            .set_text(cx, &view.entry_progress);
        let next_community_operation = view
            .community_operation
            .as_ref()
            .map(|op| (op.id.clone(), op.action.permission.clone()));
        if next_community_operation != self.community_operation
            && view.community_operation.as_ref().is_some_and(|op| {
                op.action.permission == "request_entry" && op.status == super::Status::Prepared
            })
        {
            self.tab = 6;
        }
        self.community_operation = view
            .community_operation
            .as_ref()
            .map(|op| (op.id.clone(), op.action.permission.clone()));
        self.access_nonce = view.access_nonce.clone();
        self.view
            .button(cx, ids!(access_confirm))
            .set_visible(cx, self.access_nonce.is_some());
        self.view.button(cx, ids!(entry_confirm)).set_visible(
            cx,
            view.community_operation.as_ref().is_some_and(|op| {
                op.action.permission == "request_entry" && op.status == super::Status::Prepared
            }),
        );
        self.view
            .button(cx, ids!(community_confirm_publish))
            .set_visible(
                cx,
                view.community_operation.as_ref().is_some_and(|op| {
                    op.action.permission == "publish_community"
                        && op.status == super::Status::Prepared
                }),
            );
        self.view
            .button(cx, ids!(discovery_join))
            .set_visible(cx, !view.card_status.is_empty());
        self.view.button(cx, ids!(discovery_current)).set_text(cx,
            if view.organizer { "查看当前活动" }
            else if self.participant_controls.register { "打开本场并准备报名" }
            else if self.participant_controls.reply { "查看我的邀请" }
            else { "查看我的报名" });
        self.roster_nonce = view.roster_export_nonce.clone();
        self.view
            .view(cx, ids!(roster_export))
            .set_visible(cx, view.organizer);
        self.view
            .label(cx, ids!(roster_preview))
            .set_text(cx, &view.roster_export_preview);
        self.view
            .button(cx, ids!(roster_confirm))
            .set_visible(cx, self.roster_nonce.is_some());
        let next_plan_id = view.plan.as_ref().map(|p| p.id.clone());
        if next_plan_id != self.plan_id
            && view
                .plan
                .as_ref()
                .is_some_and(|p| p.status == buwei_host_core::planning::PlanStatus::Draft)
        {
            self.creating = true;
            self.tab = 1;
        }
        self.plan_id = next_plan_id;
        self.plan_revision = view.plan.as_ref().map(|p| p.revision);
        self.plan_creation_nonce = view.plan_creation_nonce.clone();
        self.plan_choices = view.plan_choices.clone();
        self.view.label(cx, ids!(plan_summary)).set_text(
            cx,
            &view.plan.as_ref().map(|p| p.text()).unwrap_or_default(),
        );
        self.view
            .label(cx, ids!(plan_creation_preview))
            .set_text(cx, &view.plan_creation_preview);
        self.view
            .button(cx, ids!(plan_confirm_create))
            .set_visible(cx, self.plan_creation_nonce.is_some());
        macro_rules! plan_button {
            ($id:ident,$n:expr) => {
                self.view
                    .button(cx, ids!($id))
                    .set_visible(cx, self.plan_choices.get($n).is_some());
                if let Some((_, text)) = self.plan_choices.get($n) {
                    self.view.button(cx, ids!($id)).set_text(cx, text);
                }
            };
        }
        plan_button!(plan_0, 0);
        plan_button!(plan_1, 1);
        plan_button!(plan_2, 2);
        self.task_choices = view.task_choices.clone();
        self.task_page = self
            .task_page
            .min(self.task_choices.len().saturating_sub(1) / 3);
        macro_rules! task_button {
            ($id:ident, $n:expr) => {
                let item = self.task_choices.get(self.task_page * 3 + $n);
                self.view
                    .button(cx, ids!($id))
                    .set_visible(cx, item.is_some());
                if let Some((_, text)) = item {
                    self.view.button(cx, ids!($id)).set_text(
                        cx,
                        &format!("{}\n点击继续原任务", text.lines().next().unwrap_or("")),
                    );
                }
            };
        }
        task_button!(task_0, 0);
        task_button!(task_1, 1);
        task_button!(task_2, 2);
        self.view
            .view(cx, ids!(task_pagination))
            .set_visible(cx, self.task_choices.len() > 3);
        self.view
            .button(cx, ids!(pause_task))
            .set_visible(cx, !self.task_choices.is_empty());
        self.analysis_consent_id = view.analysis_consent_id.clone();
        self.view
            .label(cx, ids!(analysis_status))
            .set_text(cx, &view.analysis_status);
        self.view
            .label(cx, ids!(assistance_tasks))
            .set_text(cx, &view.assistance_tasks);
        self.view
            .label(cx, ids!(background_status))
            .set_text(cx, &view.background_status);
        self.feedback_id = view.latest_feedback.as_ref().map(|f| f.0.clone());
        self.view.label(cx, ids!(feedback_status)).set_text(
            cx,
            view.latest_feedback
                .as_ref()
                .map(|f| f.1.as_str())
                .unwrap_or(""),
        );
        self.view
            .button(cx, ids!(undo_feedback))
            .set_visible(cx, self.feedback_id.is_some());
        self.assistance_cards = view.assistance_cards.clone();
        self.assistance_page = self
            .assistance_page
            .min(self.assistance_cards.len().saturating_sub(1) / 3);
        self.view
            .view(cx, ids!(assistance_area))
            .set_visible(cx, !self.assistance_cards.is_empty());
        macro_rules! assistance_button {
            ($id:ident,$n:expr) => {
                self.view.button(cx, ids!($id)).set_visible(
                    cx,
                    self.assistance_cards.len() > self.assistance_page * 3 + $n,
                );
                if let Some(c) = self.assistance_cards.get(self.assistance_page * 3 + $n) {
                    self.view.button(cx, ids!($id)).set_text(
                        cx,
                        &format!(
                            "{}\n{}\n{}\n核验采集于 {} · 点击准备下一步",
                            c.title,
                            c.reason,
                            c.evidence,
                            buwei_host_core::calendar::display(c.observed_at)
                        ),
                    );
                }
            };
        }
        self.view.button(cx, ids!(assistance_follow)).set_visible(
            cx,
            self.assistance_cards
                .get(self.assistance_page * 3)
                .is_some_and(|c| c.action == buwei_host_core::proactive::SuggestedAction::Share),
        );
        assistance_button!(assistance_0, 0);
        assistance_button!(assistance_1, 1);
        assistance_button!(assistance_2, 2);
        if let Some(tab) = view.assistance_route {
            self.tab = tab;
            self.pages(cx);
        }
        self.goals = view.goals.clone();
        self.goal_id = view.goal_id.clone();
        self.goal_form = view.goal_form.clone();
        self.correction_id = view.goal_correction.as_ref().map(|p| p.0.clone());
        self.view.view(cx, ids!(correction_editor)).set_visible(
            cx,
            view.goal_id.is_some()
                && view
                    .goal_form
                    .as_ref()
                    .is_some_and(|f| f.kind == GoalKind::Participate),
        );
        self.view
            .view(cx, ids!(correction_confirmation))
            .set_visible(cx, view.goal_correction.as_ref().is_some_and(|p| p.2));
        self.view.label(cx, ids!(correction_diff)).set_text(
            cx,
            &view
                .goal_correction
                .as_ref()
                .map(|p| p.1.clone())
                .unwrap_or_default(),
        );
        self.view
            .label(cx, ids!(registration_progress))
            .set_text(cx, &view.goal_registration_status);
        self.goal_page = self.goal_page.min(self.goals.len().saturating_sub(1) / 3);
        macro_rules! goal_button {
            ($id:ident,$n:expr) => {
                self.view
                    .button(cx, ids!($id))
                    .set_visible(cx, self.goals.len() > self.goal_page * 3 + $n);
                if let Some((_, text)) = self.goals.get(self.goal_page * 3 + $n) {
                    self.view.button(cx, ids!($id)).set_text(cx, text);
                }
            };
        }
        goal_button!(goal_0, 0);
        goal_button!(goal_1, 1);
        goal_button!(goal_2, 2);
        self.view
            .view(cx, ids!(goal_pagination))
            .set_visible(cx, self.goals.len() > 3);
        self.view
            .label(cx, ids!(intention_result))
            .set_text(cx, &view.intention_result);
        self.view
            .label(cx, ids!(goal_sources))
            .set_text(cx, &view.goal_sources);
        self.view
            .view(cx, ids!(goal_editor))
            .set_visible(cx, view.goal_form.is_some());
        let goal_stamp = serde_json::to_string(&view.goal_form).unwrap_or_default();
        if self.last_goal_form.as_deref() != Some(goal_stamp.as_str()) {
            if let Some(f) = &view.goal_form {
                macro_rules! field {
                    ($id:ident,$value:expr) => {
                        self.view.text_input(cx, ids!($id)).set_text(cx, $value);
                    };
                }
                field!(goal_title, &f.title);
                field!(
                    goal_template,
                    buwei_host_core::assistance::template_name(&f.template)
                );
                field!(goal_earliest, &f.earliest);
                field!(goal_latest, &f.latest);
                field!(goal_group, &f.group);
                field!(goal_target, &f.target);
                field!(goal_check, &f.check_at);
                self.view
                    .view(cx, ids!(goal_organizer_fields))
                    .set_visible(cx, f.kind == GoalKind::Organize);
                self.view.view(cx, ids!(feedback_actions)).set_visible(
                    cx,
                    f.kind == GoalKind::Participate && self.goal_id.is_some(),
                );
                field!(goal_recurrence, &f.recurrence_days);
                field!(goal_preparation, &f.preparation_hours);
                self.view
                    .view(cx, ids!(goal_schedule))
                    .set_visible(cx, f.kind == GoalKind::Participate);
                self.view.label(cx, ids!(goal_schedule_status)).set_text(
                    cx,
                    &f.availability
                        .as_ref()
                        .map(|s| format!("日期范围内采用：{}", s.description()))
                        .unwrap_or("仅采用下面填写的具体时段".into()),
                );
            }
            self.last_goal_form = Some(goal_stamp);
        }
        for id in [ids!(pause_goal), ids!(resume_goal), ids!(delete_goal)] {
            self.view
                .button(cx, id)
                .set_visible(cx, self.goal_id.is_some());
        }
        let pref_stamp = serde_json::to_string(&view.personal_preferences).unwrap_or_default();
        if self.last_preferences.as_deref() != Some(pref_stamp.as_str()) {
            if let Some(p) = &view.personal_preferences {
                self.view.text_input(cx, ids!(pref_template)).set_text(
                    cx,
                    p.template
                        .as_deref()
                        .map(buwei_host_core::assistance::template_name)
                        .unwrap_or(""),
                );
                self.view
                    .text_input(cx, ids!(pref_group))
                    .set_text(cx, &p.group.map(|n| n.to_string()).unwrap_or_default());
                self.view.text_input(cx, ids!(pref_weekdays)).set_text(
                    cx,
                    &p.weekdays
                        .iter()
                        .map(|d| (d + 1).to_string())
                        .collect::<Vec<_>>()
                        .join(","),
                );
                for (path, value) in [
                    (ids!(pref_start), p.earliest_minute),
                    (ids!(pref_end), p.latest_minute),
                ] {
                    self.view.text_input(cx, path).set_text(
                        cx,
                        &value
                            .map(|m| format!("{:02}:{:02}", m / 60, m % 60))
                            .unwrap_or_default(),
                    );
                }
                self.view
                    .text_input(cx, ids!(pref_quiet_start))
                    .set_text(cx, &p.quiet_start.to_string());
                self.view
                    .text_input(cx, ids!(pref_quiet_end))
                    .set_text(cx, &p.quiet_end.to_string());
            }
            self.last_preferences = Some(pref_stamp);
        }
        self.preferences = view.personal_preferences.clone();
        self.current_activity = view.activity_identity.clone();
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
            let key = form.display_key(&view.account, self.plan_id.as_deref());
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
                self.campus_enabled = form.campus.is_some();
                let campus = form.campus.clone().unwrap_or_default();
                self.view.text_input(cx, ids!(campus_school)).set_text(cx, &campus.campus);
                self.view.text_input(cx, ids!(campus_audience)).set_text(cx, &campus.audience);
                self.view.text_input(cx, ids!(campus_topic)).set_text(cx, &campus.topic);
                self.view.text_input(cx, ids!(campus_guests)).set_text(cx, &campus.guests);
                self.view.text_input(cx, ids!(campus_requirements)).set_text(cx, &campus.requirements);
                self.view.text_input(cx, ids!(campus_deadline)).set_text(cx, &campus.signup_deadline);
                self.view.text_input(cx, ids!(campus_team)).set_text(cx, &campus.team_size.map(|n| n.to_string()).unwrap_or_default());
                self.view.text_input(cx, ids!(campus_role)).set_text(cx, campus.role.map(|r| r.name()).unwrap_or_default());
                self.last_form = Some(key);
            }
        }
        self.display_templates(cx);
        self.display_campus(cx);
        if buwei_host_core::interaction::creation_is_complete(
            self.pending_creation_plan.as_deref(), view.plan.as_ref(), &view.activity_identity,
        ) {
            self.creating = false;
            self.pending_creation_plan = None;
            self.tab = 1;
        }
        self.view
            .view(cx, ids!(creation_form))
            .set_visible(cx, self.creating && self.authorized);
        self.view
            .view(cx, ids!(organizer_new_actions))
            .set_visible(cx, view.organizer && !self.creating && self.tab==1);
        self.view.button(cx, ids!(new_activity)).set_visible(cx, self.signed_in && self.authorized && !self.creating);
        self.view.button(cx,ids!(copy_activity)).set_visible(cx,view.organizer && !self.creating);
        self.view.button(cx,ids!(archive_activity)).set_visible(cx,view.organizer && !self.creating);
        self.view
            .view(cx, ids!(organizer_invite_entry))
            .set_visible(cx, view.organizer && !self.creating && self.authorized);
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
            .set_visible(cx, false);
        self.view
            .label(cx, ids!(home_status))
            .set_text(cx, &view.automation_status);
        self.view
            .label(cx, ids!(reply_status))
            .set_visible(cx, self.authorized && self.participant_controls.reply);
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
        self.view.label(cx, ids!(account)).set_text(
            cx,
            &format!(
                "当前账号：{} · {}",
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
        self.view.label(cx, ids!(performance_summary)).set_text(cx, &view.performance_summary);
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
            .set_visible(cx, self.creating && self.authorized && self.signed_in);
        self.view.button(cx,ids!(create_with_ai)).set_text(cx,
            if self.plan_id.is_some() { "根据描述完善这份策划" } else { "准备可编辑策划稿" });
        self.view
            .button(cx, ids!(suggest))
            .set_visible(cx, !view.organizer && self.participant_controls.register && !self.creating);
        self.view
            .button(cx, ids!(clarify))
            .set_visible(cx, !view.organizer && view.has_participation_draft && !self.creating);
        self.view
            .button(cx, ids!(apply_advice))
            .set_visible(cx, !view.organizer && view.has_participation_draft && !self.creating);
        self.pages(cx);
        self.view
            .view(cx, ids!(organizer_actions))
            .set_visible(cx, view.organizer);
        self.view
            .view(cx, ids!(organizer_setup))
            .set_visible(cx, (view.organizer || self.creating) && self.authorized);
        self.view.view(cx, ids!(sharing_setup)).set_visible(cx, view.organizer && !self.creating);
        self.view.view(cx, ids!(automation_setup)).set_visible(cx, view.organizer && !self.creating);
        self.view.label(cx, ids!(activity)).set_visible(cx, self.has_activity && !self.creating);
        self.view.label(cx, ids!(people)).set_visible(cx, self.has_activity && !self.creating);
        self.view.view(cx, ids!(seat_overview)).set_visible(cx, self.has_activity && !self.creating);
        self.view.view(cx, ids!(roster_export)).set_visible(cx, view.organizer && !self.creating);
        self.view.button(cx, ids!(accept)).set_visible(cx, self.authorized && self.participant_controls.reply);
        self.view.button(cx, ids!(decline)).set_visible(cx, self.authorized && self.participant_controls.reply);
        self.view.button(cx, ids!(cancel)).set_visible(cx, self.authorized && self.participant_controls.cancel && self.tab==1);
        self.view
            .view(cx, ids!(participant_actions))
            .set_visible(cx, self.authorized && (self.participant_controls.reply || (self.tab==1 && self.participant_controls.cancel)));
        self.view
            .view(cx, ids!(participant_setup))
            .set_visible(cx, self.authorized && !self.creating && !view.organizer && self.participant_controls.register);
        self.view.view(cx, ids!(assistant_area)).set_visible(cx, self.authorized && (self.creating || (self.assistant_expanded && self.tab==1 && self.has_activity)));
        self.view.button(cx, ids!(assistant_toggle)).set_visible(cx, self.authorized && self.has_activity && !self.creating);
        self.view.label(cx, ids!(assistant_heading)).set_text(cx, if self.creating {"先说说你的想法"} else {"活动助手"});
        self.view.view(cx, ids!(requirement_field)).set_visible(cx, self.creating || (!view.organizer && self.participant_controls.register));
        self.view.label(cx, ids!(requirement_caption)).set_visible(cx, self.creating || (!view.organizer && self.participant_controls.register));
        self.view.view(cx, ids!(clarification_field)).set_visible(cx, self.plan_id.is_some() || view.dated_inputs.is_some());
        self.view.view(cx, ids!(question_field)).set_visible(cx, self.has_activity && !self.creating);
        self.view.button(cx, ids!(ask)).set_visible(cx, self.has_activity && !self.creating);
        self.view.button(cx, ids!(explain)).set_visible(cx, self.has_activity && !self.creating);
        self.view.label(cx, ids!(home_current)).set_visible(cx, false);
        macro_rules! optional_label {($id:ident,$value:expr)=>{self.view.label(cx,ids!($id)).set_visible(cx,!$value.is_empty());};}
        optional_label!(public_preview,view.public_directory_preview);
        optional_label!(community_preview,view.community_preview);
        optional_label!(community_status,view.community_status);
        optional_label!(access_preview,view.access_preview);
        optional_label!(access_status,view.access_status);
        optional_label!(entry_progress,view.entry_progress);
        optional_label!(roster_preview,view.roster_export_preview);
        optional_label!(advice,view.advice);
        self.view.label(cx,ids!(advice)).set_visible(cx,!self.creating && view.dated_inputs.is_some());
        optional_label!(ai_result,view.ai_result);
        optional_label!(share_preview,view.share_preview);
        optional_label!(policy_preview,view.policy_preview);
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
        self.view.view(cx,ids!(campus_entry)).set_visible(cx,self.creating && self.authorized);
        self.view.label(cx, ids!(profile)).set_text(cx,
            if !self.signed_in { "登录后创建或发现活动" }
            else if self.tab==1 && self.creating { "策划草稿 · 确认后才创建" }
            else if self.tab==6 { "选择范围，发现想参加的活动" }
            else if !self.has_activity { "你可以组织或参加活动" }
            else if self.current_organizer { "本场由你组织" }
            else { "本场以成员身份参加" });
        self.view.label(cx, ids!(message)).set_visible(cx, !self.creating || self.tab==1);
        self.view.label(cx, ids!(expires)).set_visible(cx,self.tab==4 && self.signed_in);
        let overview=self.tab==0 && !self.creating;
        self.view.view(cx, ids!(welcome)).set_visible(cx, overview && (!self.signed_in || !self.authorized || self.activity_count==0));
        let (title,copy)=if !self.signed_in {
            ("欢迎来到补位","从底部打开 Rinx，登录本人账号后回到这里。你可以组织活动，也可以发现适合自己的活动。")
        } else if !self.authorized {
            ("账号已连接，下一步确认权限","在账号与设置中核对身份和授权范围。确认后开始策划、报名和跟进；已有记录会继续保留。")
        } else {
            ("下一场相聚，从这里开始","点击右上方的新建活动，说说你的想法；也可以去发现活动，选择想参加的那一场。")
        };
        self.view.label(cx, ids!(welcome_title)).set_text(cx,title);
        self.view.label(cx, ids!(welcome_copy)).set_text(cx,copy);
        self.view.button(cx, ids!(welcome_settings)).set_visible(cx,!self.authorized);
        self.view.button(cx, ids!(welcome_discover)).set_visible(cx,self.authorized && self.signed_in);
        let (page,subtitle)=match self.tab {
            1 if self.creating => ("新建活动","先准备策划草稿，核对后再创建和发布。"),
            1 => ("活动详情","报名、名额与后续行动，都跟随这场活动。"),
            2 => ("活动记录","查看实际结果，沿原操作编号恢复待核实事项。"),
            3 => ("活动小记","根据核验记录准备草稿，由本人编辑并确认发布。"),
            4 => ("账号与设置","身份、权限、偏好和本机 AI，都由你管理。"),
            5 => ("我的意图","说清想做什么，让下一步与你的安排相符。"),
            6 => ("发现活动","从公开大厅或自己的社群，找到下一场相聚。"),
            _ => ("我的活动","组织下一场相聚，也找到适合自己的活动。"),
        };
        self.view.label(cx, ids!(workspace_title)).set_text(cx,page);
        self.view.label(cx, ids!(workspace_subtitle)).set_text(cx,subtitle);
        for (path,tab) in [(ids!(nav_row_activity),0),(ids!(nav_row_discover),6),(ids!(nav_row_goals),5),(ids!(nav_row_history),2),(ids!(nav_row_article),3),(ids!(nav_row_help),4)] {
            let color=if self.tab==tab || (tab==0 && self.tab==1) {[0.906,0.945,0.914,1.0]} else {[0.0,0.0,0.0,0.0]};
            self.view.view(cx,path).set_uniform(cx,id!(nav_tint),&color);
        }
        self.view.view(cx, ids!(assistant_area)).set_visible(cx,self.authorized && (self.creating || (self.assistant_expanded && self.tab==1 && self.has_activity)));
        self.view.button(cx, ids!(cancel)).set_visible(cx,self.authorized && self.participant_controls.cancel && self.tab==1);
        self.view.view(cx, ids!(participant_actions)).set_visible(cx,self.authorized && (self.participant_controls.reply || (self.tab==1 && self.participant_controls.cancel)));
        self.view
            .view(cx, ids!(assistance_area))
            .set_visible(cx, self.tab == 0 && !self.assistance_cards.is_empty());
        self.view
            .view(cx, ids!(page_goals))
            .set_visible(cx, self.tab == 5);
        self.view
            .view(cx, ids!(activity_cards))
            .set_visible(cx, overview && self.signed_in && self.activity_count>0);
        self.view.view(cx, ids!(activity_details)).set_visible(
            cx,
            (self.tab==1 && self.has_activity && self.signed_in) || self.creating,
        );
        self.view
            .button(cx, ids!(open_current))
            .set_visible(cx, false);
        self.view
            .button(cx, ids!(back_to_activities))
            .set_visible(cx, self.tab == 1);
        self.view.label(cx, ids!(activity_heading)).set_text(
            cx,
            if self.tab == 1 {
                "活动详情"
            } else {
                "我的活动"
            },
        );
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
        self.view
            .view(cx, ids!(page_discover))
            .set_visible(cx, self.tab == 6);
        self.view.redraw(cx);
    }
    fn flush_critical(&mut self) {
        if let Some(tx) = &self.sender {
            if self.pending_revoke && tx.try_send(Command::Revoke).is_ok() {
                self.pending_revoke = false;
            }
            if let Some(id) = self.pending_policy_pause.clone() {
                if tx.try_send(Command::PauseAutomationFor(id)).is_ok() {
                    self.pending_policy_pause = None;
                }
            }
        }
    }
    fn send(&mut self, cx: &mut Cx, command: Command) {
        if matches!(&command, Command::Revoke) {
            if let Some(scope) = self.scope {
                super::host::invalidate_automation(scope, None);
            }
            self.pending_revoke = true;
            self.flush_critical();
            self.view.label(cx, ids!(message)).set_text(
                cx,
                "正在撤销授权，自动发送已暂停。已发送的结果保留原编号核实。",
            );
            self.view.redraw(cx);
            return;
        }
        if matches!(&command, Command::PauseAutomation) {
            if let Some(scope) = self.scope {
                super::host::invalidate_automation(scope, Some(&self.current_activity));
            }
            self.pending_policy_pause = Some(self.current_activity.clone());
            self.flush_critical();
            self.view
                .label(cx, ids!(message))
                .set_text(cx, "自动发送已暂停，正在记录暂停结果。");
            self.view.redraw(cx);
            return;
        }
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
            self.flush_critical();
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
                if let Some(scope) = self.scope {
                    super::host::invalidate_automation(scope, Some(&self.current_activity));
                }
                self.pending_policy_pause = Some(self.current_activity.clone());
                self.flush_critical();
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
            if let Some(tab) = if clicked!(nav_activity) || clicked!(back_to_activities) {
                Some(0)
            } else if clicked!(nav_discover) || clicked!(welcome_discover) {
                Some(6)
            } else if clicked!(nav_goals) {
                Some(5)
            } else if clicked!(nav_confirm) || clicked!(open_current) || clicked!(join_card) {
                Some(1)
            } else if clicked!(nav_history) {
                Some(2)
            } else if clicked!(nav_article) {
                Some(3)
            } else if clicked!(nav_help) || clicked!(welcome_settings) {
                Some(4)
            } else {
                None
            } {
                if tab==0 { self.creating=false; }
                self.tab = tab;
                self.pages(cx);
                if !clicked!(join_card) {
                    return;
                }
            }
            if clicked!(assistant_toggle) {
                self.assistant_expanded=!self.assistant_expanded;
                self.pages(cx);
                self.view.view(cx,ids!(page_activity)).set_scroll_pos(cx,Vec2d{x:0.0,y:0.0});
                return;
            }
            if clicked!(discovery_join) || clicked!(discovery_current) {
                self.tab = 1;
                self.pages(cx);
                if clicked!(discovery_join) {
                    self.send(cx, Command::JoinCardAndPrepare);
                } else if !self.current_organizer && self.participant_controls.register {
                    self.send(cx,Command::QuickJoin);
                }
                return;
            }
            macro_rules! text {
                ($id:ident) => {
                    self.view.text_input(cx, ids!($id)).text()
                };
            }
            if [
                ids!(title),
                ids!(start),
                ids!(end),
                ids!(capacity),
                ids!(location),
                ids!(description),
            ]
            .iter()
            .any(|path| self.view.text_input(cx, *path).changed(actions).is_some())
            {
                self.plan_creation_nonce = None;
                self.view
                    .button(cx, ids!(plan_confirm_create))
                    .set_visible(cx, false);
                self.view
                    .label(cx, ids!(plan_creation_preview))
                    .set_text(cx, "基础信息已编辑，请先保存并重新预览创建。");
            }
            if clicked!(enable_background) {
                if super::tray::enable() {
                    super::host::set_background(true);
                    self.send(cx, Command::Refresh);
                } else {
                    self.view
                        .label(cx, ids!(message))
                        .set_text(cx, "托盘未成功开启，窗口保持打开；请重试。");
                }
                return;
            }
            if clicked!(pause_background) {
                super::host::pause_background();
                self.send(cx, Command::Revoke);
                return;
            }
            if clicked!(disable_background) {
                super::host::set_background(false);
                super::tray::disable();
                self.send(cx, Command::Refresh);
                return;
            }
            if clicked!(assistance_prev) || clicked!(assistance_next) {
                if clicked!(assistance_prev) {
                    self.assistance_page = self.assistance_page.saturating_sub(1);
                } else {
                    self.assistance_page = (self.assistance_page + 1)
                        .min(self.assistance_cards.len().saturating_sub(1) / 3);
                }
                self.send(cx, Command::Refresh);
                return;
            }
            if let Some(index) = if clicked!(assistance_0) {
                Some(0)
            } else if clicked!(assistance_1) {
                Some(1)
            } else if clicked!(assistance_2) {
                Some(2)
            } else {
                None
            } {
                if let Some(c) = self.assistance_cards.get(self.assistance_page * 3 + index) {
                    self.send(
                        cx,
                        Command::Assistance(IntentCommand::UseCard {
                            id: c.id.clone(),
                            fingerprint: c.fingerprint.clone(),
                        }),
                    );
                }
                return;
            }
            if clicked!(assistance_follow) {
                if let Some(c) = self.assistance_cards.get(self.assistance_page * 3) {
                    self.send(
                        cx,
                        Command::Assistance(IntentCommand::FollowCard {
                            id: c.id.clone(),
                            fingerprint: c.fingerprint.clone(),
                        }),
                    );
                }
                return;
            }
            if clicked!(assistance_snooze)
                || clicked!(assistance_ignore)
                || clicked!(assistance_disable)
            {
                if let Some(c) = self.assistance_cards.get(self.assistance_page * 3) {
                    self.send(
                        cx,
                        Command::Assistance(IntentCommand::ControlCard {
                            id: c.id.clone(),
                            snooze: clicked!(assistance_snooze),
                            disable: clicked!(assistance_disable),
                        }),
                    );
                }
                return;
            }
            if clicked!(goal_prev) || clicked!(goal_next) {
                if clicked!(goal_prev) {
                    self.goal_page = self.goal_page.saturating_sub(1);
                } else {
                    self.goal_page =
                        (self.goal_page + 1).min(self.goals.len().saturating_sub(1) / 3);
                }
                self.send(cx, Command::Refresh);
                return;
            }
            if clicked!(task_prev) || clicked!(task_next) {
                self.task_page = if clicked!(task_prev) {
                    self.task_page.saturating_sub(1)
                } else {
                    (self.task_page + 1).min(self.task_choices.len().saturating_sub(1) / 3)
                };
                self.send(cx, Command::Refresh);
                return;
            }
            if clicked!(pause_task) {
                if let Some((id, _)) = self.task_choices.get(self.task_page * 3) {
                    self.send(
                        cx,
                        Command::Assistance(IntentCommand::PauseTask(id.clone())),
                    );
                }
                return;
            }
            if let Some(n) = if clicked!(task_0) {
                Some(0)
            } else if clicked!(task_1) {
                Some(1)
            } else if clicked!(task_2) {
                Some(2)
            } else {
                None
            } {
                if let Some((id, _)) = self.task_choices.get(self.task_page * 3 + n) {
                    self.send(
                        cx,
                        Command::Assistance(IntentCommand::ResumeTask(id.clone())),
                    );
                }
                return;
            }
            if let Some(n) = if clicked!(goal_0) {
                Some(0)
            } else if clicked!(goal_1) {
                Some(1)
            } else if clicked!(goal_2) {
                Some(2)
            } else {
                None
            } {
                if let Some((id, _)) = self.goals.get(self.goal_page * 3 + n) {
                    self.send(
                        cx,
                        Command::Assistance(IntentCommand::SelectGoal(id.clone())),
                    );
                }
                return;
            }
            if clicked!(new_activity) {
                self.creating = true;
                self.pending_creation_plan = None;
                self.plan_id = None;
                self.plan_revision = None;
                self.send(
                    cx,
                    Command::Planning(super::controller::planning::PlanningCommand::New),
                );
                self.tab = 1;
                self.pages(cx);
                self.view
                    .view(cx, ids!(creation_form))
                    .set_visible(cx, true);
                return;
            }
            if clicked!(campus_toggle) {
                self.campus_enabled = !self.campus_enabled;
                self.display_campus(cx);
                self.view.redraw(cx);
                return;
            }
            if clicked!(campus_role_audience) || clicked!(campus_role_performer) {
                self.view.text_input(cx, ids!(campus_role)).set_text(cx,
                    if clicked!(campus_role_audience) { "观众报名" } else { "演出招募" });
                self.view.redraw(cx);
                return;
            }
            let campus_kind = if clicked!(campus_lecture) { Some("lecture") }
                else if clicked!(campus_competition) { Some("competition") }
                else if clicked!(campus_movie) { Some("movie") }
                else if clicked!(campus_festival) { Some("music_festival") }
                else if clicked!(campus_club) { Some("club") } else { None };
            if let Some(kind) = campus_kind {
                self.creating = true;
                self.pending_creation_plan = None;
                self.tab = 1;
                self.send(cx, Command::UseCampusTemplate(kind.into()));
                self.pages(cx);
                return;
            }
            if let Some(query)=self.view.text_input(cx,ids!(community_search)).changed(actions) {
                self.community_query=query;self.community_page=0;
                self.display_communities(cx);self.view.redraw(cx);return;
            }
            if clicked!(community_search_clear) || clicked!(community_prev) || clicked!(community_next) {
                if clicked!(community_search_clear) {
                    self.community_query.clear();self.community_page=0;
                    self.view.text_input(cx,ids!(community_search)).set_text(cx,"");
                } else if clicked!(community_prev) { self.community_page=self.community_page.saturating_sub(1); }
                else { self.community_page+=1; }
                self.display_communities(cx);self.view.redraw(cx);return;
            }
            if let Some(query) = self.view.text_input(cx, ids!(template_search)).changed(actions) {
                self.template_query = query;
                self.template_page = 0;
                self.display_templates(cx);
                self.view.redraw(cx);
            }
            let template_search_entered = self.view.text_input(cx, ids!(template_search)).returned(actions).is_some();
            if clicked!(templates_toggle) || clicked!(template_search_apply) || template_search_entered
                || clicked!(template_search_clear) || clicked!(template_prev) || clicked!(template_next)
            {
                if clicked!(templates_toggle) {
                    self.template_library_open = !self.template_library_open;
                }
                if clicked!(template_search_apply) || template_search_entered || clicked!(template_search_clear) {
                    self.template_query = if clicked!(template_search_clear) { String::new() }
                        else { text!(template_search) };
                    self.template_page = 0;
                    self.view.text_input(cx, ids!(template_search)).set_text(cx, &self.template_query);
                }
                if clicked!(template_prev) { self.template_page = self.template_page.saturating_sub(1); }
                if clicked!(template_next) { self.template_page += 1; }
                self.display_templates(cx);
                self.view.redraw(cx);
                return;
            }
            let template_option = if clicked!(template_option_0) { Some(0) }
                else if clicked!(template_option_1) { Some(1) }
                else if clicked!(template_option_2) { Some(2) }
                else if clicked!(template_option_3) { Some(3) }
                else if clicked!(template_option_4) { Some(4) }
                else if clicked!(template_option_5) { Some(5) } else { None };
            if let Some(n) = template_option {
                if let Some(t) = buwei_host_core::activity_types::search(&self.template_query).get(self.template_page * 6 + n) {
                    self.creating = true;
                    self.tab = 1;
                    self.template_library_open = false;
                    self.send(cx, Command::UseTemplate(t.id.into()));
                    self.pages(cx);
                }
                return;
            }
            if clicked!(cancel_setup) {
                self.creating = false;
                self.pending_creation_plan = None;
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
                self.tab = 1;
                self.pages(cx);
            }
            if clicked!(plan_confirm_create) {
                self.pending_creation_plan = self.plan_id.clone();
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
                self.tab = 1;
                self.pages(cx);
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
                use super::controller::community_access::DiscoveryCommand as D;
                if clicked!(public_open) {return Ok(Some(Command::Discovery(D::PreviewPublicDirectory)));}
                if clicked!(public_confirm) {return Ok(Some(Command::Discovery(D::ConfirmPublicDirectory(self.public_nonce.clone().ok_or("请先核对公开大厅范围")?))));}
                if clicked!(community_mode) {self.selected_community=None;return Ok(Some(Command::Discovery(D::CommunityMode)));}
                if clicked!(quick_join) {return Ok(Some(Command::QuickJoin));}
                if clicked!(communities_load) {
                    return Ok(Some(Command::Discovery(D::LoadCommunities)));
                }
                if clicked!(communities_refresh) {
                    return Ok(Some(Command::Discovery(D::Refresh)));
                }
                if clicked!(listing_prev) || clicked!(listing_next) {
                    self.listing_page = if clicked!(listing_prev) {
                        self.listing_page.saturating_sub(1)
                    } else {
                        (self.listing_page + 1).min(self.listing_keys.len().saturating_sub(1) / 3)
                    };
                    return Ok(Some(Command::Refresh));
                }
                if clicked!(community_0) || clicked!(community_1) || clicked!(community_2) {
                    let n = if clicked!(community_0) {
                        0
                    } else if clicked!(community_1) {
                        1
                    } else {
                        2
                    };
                    let (id, label) = self
                        .communities
                        .get(self.community_page * 3 + n)
                        .ok_or("请刷新社群列表")?;
                    self.selected_community = Some(id.clone());
                    self.view
                        .label(cx, ids!(community_selected))
                        .set_text(cx, &format!("当前选择：{label}"));
                    return Ok(Some(Command::Discovery(D::Watch(id.clone()))));
                }
                if clicked!(filter_apply) || clicked!(filter_available) || clicked!(filter_campus) {
                    if clicked!(filter_campus) {
                        self.view.text_input(cx, ids!(filter_template)).set_text(cx, "校园");
                    }
                    if clicked!(filter_available) {
                        self.available_only = !self.available_only;
                        self.view.button(cx, ids!(filter_available)).set_text(
                            cx,
                            if self.available_only {
                                "已筛选：发布时有空位"
                            } else {
                                "只看发布时有空位"
                            },
                        );
                    }
                    return Ok(Some(Command::Discovery(D::Filter(
                        buwei_host_core::discovery::Filter {
                            template: if text!(filter_template).trim().is_empty() {
                                String::new()
                            } else if matches!(text!(filter_template).trim(), "校园" | "校园活动" | "campus") {
                                "campus".into()
                            } else {
                                super::controller::intentions::parse_template(&text!(
                                    filter_template
                                ))?
                            },
                            date: text!(filter_date),
                            available_only: self.available_only,
                        },
                    ))));
                }
                if clicked!(listing_0) || clicked!(listing_1) || clicked!(listing_2) {
                    let n = if clicked!(listing_0) {
                        0
                    } else if clicked!(listing_1) {
                        1
                    } else {
                        2
                    };
                    return Ok(Some(Command::Discovery(D::PreviewEntry(
                        self.listing_keys
                            .get(self.listing_page * 3 + n)
                            .ok_or("请刷新社群活动")?
                            .clone(),
                    ))));
                }
                if clicked!(community_publish) || clicked!(community_withdraw) {
                    if self.public_mode {return Ok(Some(Command::Discovery(D::PreviewPublicPublish {withdrawn:clicked!(community_withdraw)})));}
                    return Ok(Some(Command::Discovery(D::PreviewPublish {
                        community: self.selected_community.clone().ok_or("请先选择社群")?,
                        withdrawn: clicked!(community_withdraw),
                    })));
                }
                if clicked!(community_confirm_publish) || clicked!(entry_confirm) {
                    let (id, permission) = self
                        .community_operation
                        .as_ref()
                        .ok_or("请先预览本次社群操作")?;
                    return Ok(Some(Command::Discovery(
                        if permission == "publish_community" {
                            D::ConfirmPublish(id.clone())
                        } else {
                            D::ConfirmEntry(id.clone())
                        },
                    )));
                }
                if clicked!(access_prepare) {
                    return Ok(Some(Command::Discovery(D::PreviewAccess(format!(
                        "{}|{}",
                        self.selected_community.clone().ok_or("请先选择社群")?,
                        self.current_activity
                    )))));
                }
                if clicked!(access_confirm) {
                    return Ok(Some(Command::Discovery(D::ConfirmAccess(
                        self.access_nonce.clone().ok_or("请先查看加入规则")?,
                    ))));
                }
                if clicked!(access_pause) {
                    if let Some(scope) = self.scope {
                        super::host::invalidate_community(scope);
                    }
                    return Ok(Some(Command::Discovery(D::PauseAccess)));
                }
                if clicked!(community_reconcile) {
                    return Ok(Some(Command::Discovery(D::Reconcile)));
                }
                if clicked!(roster_prepare) {
                    return Ok(super::controller::roster_export::choose_path()
                        .map(Command::PreviewRosterExport));
                }
                if clicked!(roster_confirm) {
                    return Ok(Some(Command::ConfirmRosterExport(
                        self.roster_nonce.clone().ok_or("请先预览名单导出")?,
                    )));
                }
                if clicked!(plan_0) || clicked!(plan_1) || clicked!(plan_2) {
                    let n = if clicked!(plan_0) {
                        0
                    } else if clicked!(plan_1) {
                        1
                    } else {
                        2
                    };
                    return Ok(Some(Command::Planning(
                        super::controller::planning::PlanningCommand::Select(
                            self.plan_choices.get(n).ok_or("请刷新策划列表")?.0.clone(),
                        ),
                    )));
                }
                if clicked!(plan_revise) {
                    return Ok(Some(Command::Planning(
                        super::controller::planning::PlanningCommand::Prepare {
                            text: if text!(clarification).trim().is_empty() {
                                text!(requirement)
                            } else {
                                text!(clarification)
                            },
                            id: self.plan_id.clone(),
                            revision: self.plan_revision,
                        },
                    )));
                }
                if clicked!(plan_preview_create) {
                    return Ok(Some(Command::Planning(
                        super::controller::planning::PlanningCommand::PreviewCreate,
                    )));
                }
                if clicked!(plan_confirm_create) {
                    return Ok(Some(Command::Planning(
                        super::controller::planning::PlanningCommand::ConfirmCreate {
                            nonce: self
                                .plan_creation_nonce
                                .clone()
                                .ok_or("请先保存并预览创建")?,
                            basics: buwei_host_core::planning::ActivityBasics {
                                title: text!(title),
                                capacity: parse(text!(capacity))?,
                                start: text!(start),
                                end: text!(end),
                                template: self.template.clone(),
                                location: text!(location),
                                description: text!(description),
                                campus: self.campus_input(cx)?,
                            },
                        },
                    )));
                }
                if clicked!(correction_prepare)
                    || clicked!(correction_once)
                    || clicked!(correction_long)
                    || clicked!(registration_update)
                {
                    let saved = self.goal_form.as_ref().ok_or("请先选择已保存的目标")?;
                    if !saved.participation_fields_match(
                        &text!(goal_title),
                        &text!(goal_template),
                        &text!(goal_earliest),
                        &text!(goal_latest),
                        &text!(goal_group),
                    ) {
                        return Err("目标有尚未保存的修改，请先点击“确认保存目标”，再准备更正或预览候补更新。".into());
                    }
                }
                Ok(
                    if clicked!(new_organize_goal) || clicked!(new_participate_goal) {
                        Some(Command::Assistance(IntentCommand::PrepareGoal(
                            if clicked!(new_organize_goal) {
                                GoalKind::Organize
                            } else {
                                GoalKind::Participate
                            },
                        )))
                    } else if clicked!(goal_ai_organize) || clicked!(goal_ai_participate) {
                        Some(Command::Assistance(IntentCommand::PrepareGoalWithAi {
                            text: text!(goal_request),
                            kind: if clicked!(goal_ai_organize) {
                                GoalKind::Organize
                            } else {
                                GoalKind::Participate
                            },
                        }))
                    } else if clicked!(correction_prepare) {
                        Some(Command::Assistance(IntentCommand::PrepareGoalCorrection {
                            id: self.goal_id.clone().ok_or("请先选择目标")?,
                            text: text!(correction_request),
                        }))
                    } else if clicked!(correction_once) || clicked!(correction_long) {
                        Some(Command::Assistance(IntentCommand::ConfirmGoalCorrection {
                            id: self.correction_id.clone().ok_or("请先核对更正差异")?,
                            scope: if clicked!(correction_once) {
                                buwei_host_core::intent_feedback::FeedbackScope::ThisOccasion
                            } else {
                                buwei_host_core::intent_feedback::FeedbackScope::LongTerm
                            },
                        }))
                    } else if clicked!(registration_update) {
                        Some(Command::Assistance(
                            IntentCommand::PrepareGoalRegistrationUpdate(
                                self.goal_id.clone().ok_or("请先选择目标")?,
                            ),
                        ))
                    } else if clicked!(save_goal)
                        || clicked!(goal_weekly)
                        || clicked!(goal_specific)
                    {
                        let mut f = self.goal_form.clone().ok_or("请先新建或选择目标")?;
                        f.title = text!(goal_title);
                        f.template = text!(goal_template);
                        f.earliest = text!(goal_earliest);
                        f.latest = text!(goal_latest);
                        f.group = text!(goal_group);
                        if f.kind == GoalKind::Organize {
                            f.target = text!(goal_target);
                            f.check_at = text!(goal_check);
                            f.recurrence_days = text!(goal_recurrence);
                            f.preparation_hours = text!(goal_preparation);
                        }
                        if clicked!(goal_weekly) || clicked!(goal_specific) {
                            Some(Command::Assistance(IntentCommand::SetGoalAvailability {
                                form: f,
                                weekly: clicked!(goal_weekly),
                            }))
                        } else {
                            Some(Command::Assistance(IntentCommand::SaveGoal {
                                id: self.goal_id.clone(),
                                form: f,
                            }))
                        }
                    } else if clicked!(pause_goal) || clicked!(resume_goal) {
                        Some(Command::Assistance(IntentCommand::SetGoalStatus {
                            id: self.goal_id.clone().ok_or("请先选择目标")?,
                            status: if clicked!(pause_goal) {
                                GoalStatus::Paused
                            } else {
                                GoalStatus::Active
                            },
                        }))
                    } else if clicked!(delete_goal) {
                        Some(Command::Assistance(IntentCommand::DeleteGoal(
                            self.goal_id.clone().ok_or("请先选择目标")?,
                        )))
                    } else if clicked!(feedback_once) || clicked!(feedback_long) {
                        Some(Command::Assistance(IntentCommand::Feedback {
                            goal_id: self.goal_id.clone().ok_or("请先保存本次目标")?,
                            template: text!(goal_template),
                            group: parse(text!(goal_group))?,
                            scope: if clicked!(feedback_once) {
                                buwei_host_core::intent_feedback::FeedbackScope::ThisOccasion
                            } else {
                                buwei_host_core::intent_feedback::FeedbackScope::LongTerm
                            },
                        }))
                    } else if clicked!(undo_feedback) {
                        Some(Command::Assistance(IntentCommand::UndoFeedback(
                            self.feedback_id.clone().ok_or("没有可撤回反馈")?,
                        )))
                    } else if clicked!(save_preferences) {
                        let minute = |s: String| -> super::Result<Option<u16>> {
                            if s.trim().is_empty() {
                                return Ok(None);
                            }
                            let (h, m) = s.trim().split_once(':').ok_or("可用时段请填写 HH:MM")?;
                            let h: u16 = h.parse().map_err(|_| "小时不合法")?;
                            let m: u16 = m.parse().map_err(|_| "分钟不合法")?;
                            if h > 24 || m > 59 || (h == 24 && m > 0) {
                                return Err("可用时段不合法".into());
                            }
                            Ok(Some(h * 60 + m))
                        };
                        let template = if text!(pref_template).trim().is_empty() {
                            None
                        } else {
                            Some(super::controller::intentions::parse_template(&text!(
                                pref_template
                            ))?)
                        };
                        let weekdays = if text!(pref_weekdays).trim().is_empty() {
                            vec![]
                        } else {
                            text!(pref_weekdays)
                                .replace('，', ",")
                                .split(',')
                                .map(|s| {
                                    s.trim()
                                        .parse::<u8>()
                                        .ok()
                                        .filter(|d| (1..=7).contains(d))
                                        .map(|d| d - 1)
                                        .ok_or("星期请填写 1–7，以逗号分隔")
                                })
                                .collect::<Result<Vec<_>, _>>()?
                        };
                        Some(Command::Assistance(IntentCommand::SavePreferences(
                            PersonalPreferences {
                                template,
                                group: if text!(pref_group).trim().is_empty() {
                                    None
                                } else {
                                    Some(parse(text!(pref_group))?)
                                },
                                weekdays,
                                earliest_minute: minute(text!(pref_start))?,
                                latest_minute: minute(text!(pref_end))?,
                                quiet_start: parse(text!(pref_quiet_start))?,
                                quiet_end: parse(text!(pref_quiet_end))?,
                                reminders: self.preferences.as_ref().is_none_or(|p| p.reminders),
                                confirmed_at: 0,
                            },
                        )))
                    } else if clicked!(reset_preferences) {
                        Some(Command::Assistance(IntentCommand::ResetPreferences))
                    } else if clicked!(preview_analysis) {
                        Some(Command::Assistance(IntentCommand::PreviewAnalysis))
                    } else if clicked!(confirm_analysis) {
                        Some(Command::Assistance(IntentCommand::ConfirmAnalysis(
                            self.analysis_consent_id
                                .clone()
                                .ok_or("请先查看主动分析说明")?,
                        )))
                    } else if clicked!(revoke_analysis) {
                        if let Some(scope) = self.scope {
                            super::host::invalidate_automation(scope, Some("_intent_ai"));
                        }
                        Some(Command::Assistance(IntentCommand::RevokeAnalysis))
                    } else if clicked!(authorize) {
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
                        Some(Command::Planning(
                            super::controller::planning::PlanningCommand::SaveAndPreview {
                                id: self.plan_id.clone(),
                                revision: self.plan_revision,
                                basics: buwei_host_core::planning::ActivityBasics {
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
                                    campus: self.campus_input(cx)?,
                                },
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
                        Some(Command::Planning(super::controller::planning::PlanningCommand::Prepare {
                            text: text!(requirement), id: self.plan_id.clone(), revision: self.plan_revision,
                        }))
                    } else if clicked!(ask) {
                        Some(Command::Ask(text!(question)))
                    } else if clicked!(join) {
                        Some(Command::Join(Preferences {
                            earliest: time(text!(earliest))?,
                            latest: time(text!(latest))?,
                            group: parse(text!(group))?,
                        }))
                    } else if clicked!(participant_confirm) {
                        Some(Command::ConfirmParticipant(if self.participant_join_preview {
                            Some(Preferences {
                                earliest: time(text!(earliest))?,
                                latest: time(text!(latest))?,
                                group: parse(text!(group))?,
                            })
                        } else {
                            None
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
                    },
                )
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
            executor: Box::new(BuWeiExecutor {
                scope,
                replies: handles.replies,
                pending: Default::default(),
            }),
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
struct BuWeiExecutor {
    scope: makepad_app_module::InstanceScope,
    replies: makepad_app_module::ReplySink,
    pending: std::collections::HashMap<String, std::sync::Arc<std::sync::atomic::AtomicBool>>,
}
impl ServiceExecutor for BuWeiExecutor {
    fn manifest(&self) -> ServiceManifest {
        super::planning_service::manifest()
    }
    fn execute(&mut self, _cx: &mut Cx, call: &ServiceCall) -> ExecOutcome {
        self.pending.retain(|_, token| !token.load(std::sync::atomic::Ordering::SeqCst));
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        match super::host::plan_tool(self.scope, call, self.replies.clone(), cancelled.clone()) {
            Ok(()) => {
                self.pending.insert(call.call_id.clone(), cancelled);
                ExecOutcome::Pending
            }
            Err(e) => ExecOutcome::Done(ToolResult::refused(&call.call_id, e)),
        }
    }
    fn cancel(&mut self, _cx: &mut Cx, id: &str) {
        if let Some(token) = self.pending.remove(id) {
            token.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
}
