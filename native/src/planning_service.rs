//! Scoped native tools edit local plans; only the UI can create or share.
use super::*;
use makepad_app_module::makepad_ai_services::wire::{
    Risk, ServiceCall, ServiceManifest, ToolDef, ToolResult,
};
use makepad_app_module::{InstanceScope, ReplySink};
use std::sync::atomic::{AtomicBool, Ordering};
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposalArgs {
    text: String,
    draft_id: Option<String>,
    revision: Option<u64>,
    proposal: buwei_host_core::planning::PlanProposal,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadArgs {
    draft_id: Option<String>,
}
pub(crate) enum Request {
    Proposal(controller::planning::PlanningCommand),
    Read(Option<String>),
}
pub(crate) struct Call {
    pub scope: InstanceScope,
    pub actor: String,
    pub call_id: String,
    pub request: Request,
    pub cancelled: Arc<AtomicBool>,
    pub replies: ReplySink,
}
pub(crate) fn manifest() -> ServiceManifest {
    let proposal = json!({"type":"object","additionalProperties":false,"required":["text","draft_id","revision","proposal"],"properties":{"text":{"type":"string","maxLength":4000},"draft_id":{"type":["string","null"]},"revision":{"type":["integer","null"]},"proposal":{"type":"object","additionalProperties":false,"required":["updates","agenda","preparation","recruitment","reminders"],"properties":community_model::plan_properties()}}});
    let read = json!({"type":"object","additionalProperties":false,"required":["draft_id"],"properties":{"draft_id":{"type":["string","null"]}}});
    let types = buwei_host_core::activity_types::model_catalog();
    let description = format!("将本人原话整理成可编辑策划稿。新稿 draft_id/revision=null；先读取原稿再修改。updates只含本次提及字段，source逐字引用text。日期为北京时间YYYY-MM-DD HH:MM，capacity为数字字符串。template从以下目录选择最具体的id，未收录类型使用custom：{types}。校园活动还支持campus.campus学校校区、campus.audience面向人群、campus.topic主题影片场次、campus.guests已确认嘉宾主讲、campus.requirements参加须知、campus.signup_deadline北京报名截止、campus.team_size竞赛每份报名人数1–8、campus.role音乐节用途audience或performer，须引用观众报名或演出招募原话。前五项value须原样包含于source；截止与开始分别提取，竞赛容量按人数，不按队数，缺少每队人数由应用追问。流程、准备、招募文案、提醒只作建议。工具不会创建、分享、订票或报名。");
    ServiceManifest::new("buwei","补位","活动策划与报名助手；工具只准备本账号的策划稿，实际创建和分享由本人在应用中确认。")
        .with_tool(ToolDef::new("prepare_activity_plan",&description,proposal.to_string(),Risk::Act).with_preview())
        .with_tool(ToolDef::new("get_activity_plan","读取本人当前策划稿与版本，供修改前核对。draft_id=null读取当前稿。",read.to_string(),Risk::Read))
}
pub(crate) const SYSTEM_TOOLS: &[(&str, &str)] = &[
    ("buwei", "buwei.prepare_activity_plan"),
    ("buwei", "buwei.get_activity_plan"),
];
pub(crate) fn system_declarations() -> Vec<Value> {
    manifest().tools.into_iter().map(|tool| {
        let name = format!("buwei.{}", tool.name);
        let risk = if name == "buwei.get_activity_plan" { "read" } else { "act" };
        json!({"name":name,"description":tool.description,"input_schema":serde_json::from_str::<Value>(&tool.parameters).expect("constant plan schema"),"risk":risk,"confirm":"host","shareable":true})
    }).collect()
}
pub(crate) fn parse(call: &ServiceCall, scope: InstanceScope) -> Result<Request> {
    if call.args.len() > 18000 || call.call_id.len() > 160 {
        return Err("工具输入超出范围".into());
    }
    match call.tool.as_str() {
        "prepare_activity_plan" => {
            let a: ProposalArgs =
                serde_json::from_str(&call.args).map_err(|_| "策划工具参数不合法")?;
            if a.draft_id.is_some() != a.revision.is_some() {
                return Err("修改稿必须带原编号和版本".into());
            }
            Ok(Request::Proposal(
                controller::planning::PlanningCommand::Proposal {
                    text: a.text,
                    id: a.draft_id,
                    revision: a.revision,
                    proposal: a.proposal,
                    call_id: Some(format!("{scope}:{}", call.call_id)),
                },
            ))
        }
        "get_activity_plan" => {
            let a: ReadArgs = serde_json::from_str(&call.args).map_err(|_| "读取工具参数不合法")?;
            Ok(Request::Read(a.draft_id))
        }
        _ => Err("没有此策划工具；实际执行请在应用中确认".into()),
    }
}
pub(crate) fn run(c: &mut controller::Controller, call: Call) -> Option<controller::View> {
    if call.cancelled.load(Ordering::SeqCst) {
        call.replies.reply(ToolResult::cancelled(call.call_id));
        return None;
    }
    let result = c.planning_tool(&call.actor, call.request);
    let reply=match result {Ok(p)=>ToolResult::ok(&call.call_id,"策划稿已准备，请在补位中核对基础信息并另行确认创建。","本机策划稿").with_data(json!({"draft_id":p.id,"revision":p.revision,"goal":p.goal,"basics":p.basics,"agenda":p.agenda,"preparation":p.preparation,"recruitment":p.recruitment,"reminders":p.reminders,"questions":p.basics.questions(),"requires_user_confirmation":true,"matrix_actions_performed":0}).to_string()).with_preview(),Err(e)=>ToolResult::refused(&call.call_id,e)};
    call.replies.reply(reply);
    call.cancelled.store(true, Ordering::SeqCst);
    Some(c.view())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manifest_and_parser_expose_no_execution_or_identity_arguments() {
        let m = manifest();
        assert_eq!(m.tools.len(), 2);
        assert!(m.tools.iter().all(|t| !t.name.contains("create_room")));
        let s = InstanceScope::new(1, 1);
        let call = ServiceCall {
            call_id: "x".into(),
            tool: "get_activity_plan".into(),
            args: r#"{"draft_id":null,"account":"@evil:test"}"#.into(),
        };
        assert!(parse(&call, s).is_err());
        let call = ServiceCall {
            tool: "create_room".into(),
            ..call
        };
        assert!(parse(&call, s).is_err());
    }
    #[test]
    fn system_tools_are_only_scoped_draft_tools_with_host_confirmation() {
        let declarations=system_declarations();
        assert_eq!(declarations.len(),2);
        for declaration in declarations {
            assert!(SYSTEM_TOOLS.iter().any(|(_,name)| *name==declaration["name"].as_str().unwrap()));
            assert_eq!(declaration["confirm"],"host");
            assert_eq!(declaration["input_schema"]["additionalProperties"],false);
            assert!(declaration["input_schema"]["properties"].get("account").is_none());
        }
    }
    #[test]
    fn packaged_system_tool_grants_reject_unlinked_and_foreign_owners() {
        assert_eq!(octosense_shell::ext::system_tools_given(SYSTEM_TOOLS, &["buwei"]).len(), 2);
        assert!(octosense_shell::ext::system_tools_given(SYSTEM_TOOLS, &[]).is_empty());
        let forged=[("buwei","terminal.run"),("other","other.read"),("buwei","buwei.nested.run")];
        assert!(octosense_shell::ext::system_tools_given(&forged,&["buwei"]).is_empty());
    }
    #[test]
    fn system_catalog_resolves_the_linked_module_without_namespace_poisoning() {
        fn linked() -> Vec<&'static dyn makepad_app_module::AppModule> { vec![&crate::gui::MODULE] }
        octosense_shell::ext::install(octosense_shell::ext::Ext {linked_modules:linked,trusted_module:|_|false});
        let mut catalog=octosense_shell::host_tools::relay::Catalog::default();
        catalog.declare("org.example.fake",system_declarations());
        assert_eq!(catalog.owner_of("buwei.get_activity_plan"),Some("buwei".into()));
        assert!(catalog.entry("buwei","buwei.get_activity_plan").is_none());
        catalog.declare("buwei",system_declarations());
        assert_eq!(catalog.entry("buwei","buwei.get_activity_plan").unwrap()["risk"],"read");
    }
    #[test]
    fn tool_enums_have_explicit_types_required_by_k3() {
        fn verify(value:&Value) {
            match value {
                Value::Object(object) => {
                    if object.contains_key("enum") {assert!(object.contains_key("type"),"K3 tool enums require an explicit type");}
                    for child in object.values(){verify(child);}
                }
                Value::Array(values) => for child in values {verify(child);},
                _ => {}
            }
        }
        for declaration in system_declarations(){verify(&declaration["input_schema"]);}
    }
}
