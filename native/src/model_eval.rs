//! Deliberate development runs using synthetic cases and the official ModelHost.
//! No Matrix session is opened, no event is sent, and no business store is used.
use super::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, io::Write, time::Instant};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Suite { data_class: String, cases: Vec<Case> }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case { id: String, #[serde(rename="task")] kind: CaseKind }
#[derive(Deserialize)]
#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
enum CaseKind {
    Preference { turns: Vec<String>, expected: Expected },
    Explain { scenario: Scenario },
    Note { scenario: Scenario },
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Expected { ready: bool, #[serde(default)] preferences: Option<Preferences> }
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all="snake_case")]
enum Scenario { Empty, Eligible, Rejoined, GroupBlocked, TimeBlocked, Paused, Full, Reserved, Thirty }

fn synthetic_activity(scenario: Scenario) -> Result<Activity> {
    use buwei_host_core::{PersonStatus,Invitation,Delivery,Reply};
    let capacity=if matches!(scenario,Scenario::Thirty){30}else{1};
    let mut a=Activity::new("@organizer:example.invalid".into(),"!synthetic".into(),"合成活动".into(),capacity,19,21)?;
    if matches!(scenario,Scenario::Empty){return Ok(a);}
    let preferences=Preferences{earliest:17,latest:23,group:1};
    a.add_person("@first:example.invalid".into(),"合成参与者甲".into(),preferences.clone(),true)?;
    if matches!(scenario,Scenario::Rejoined|Scenario::GroupBlocked|Scenario::TimeBlocked){
        a.add_person("@second:example.invalid".into(),"合成参与者乙".into(),preferences.clone(),true)?;
    }
    match scenario {
        Scenario::Rejoined=>{a.people[0].status=PersonStatus::Cancelled;a.join_own("@first:example.invalid".into(),"合成参与者甲".into(),preferences)?;},
        Scenario::GroupBlocked=>a.people[0].preferences.group=2,
        Scenario::TimeBlocked=>a.people[0].preferences.latest=20,
        Scenario::Paused=>a.paused=true,
        Scenario::Full=>a.people[0].status=PersonStatus::Confirmed,
        Scenario::Reserved=>a.invitations.push(Invitation{operation_id:"0".repeat(32),recipient:a.people[0].account.clone(),room:a.room.clone(),digest:"0".repeat(64),until:now()+300,delivery:Delivery::Delivered,server_event:Some("$synthetic".into()),reply:Reply::Pending}),
        Scenario::Thirty=>for n in 2..=30 {a.add_person(format!("@person{n}:example.invalid"),format!("合成参与者{n}"),preferences.clone(),true)?;},
        _=>{},
    }
    a.validate()?;Ok(a)
}

fn save_report(path:&Path,report:&Value)->Result<()> {
    if let Some(parent)=path.parent(){std::fs::create_dir_all(parent).map_err(|_|"测试记录目录不可写")?;}
    let temporary=path.with_extension("write.tmp");let mut file=std::fs::File::create(&temporary).map_err(|_|"测试记录不可保存")?;
    file.write_all(&serde_json::to_vec_pretty(report).map_err(|_|"测试记录不合法")?).map_err(|_|"测试记录不可保存")?;file.sync_all().map_err(|_|"测试记录不可保存")?;drop(file);
    std::fs::rename(temporary,path).map_err(|_|"测试记录不可保存".into())
}
fn provider_budget(root:&Path)->Result<Value> {
    let path=root.join("data/model/development-budget.json");
    if path.is_file(){serde_json::from_slice(&std::fs::read(path).map_err(|_|"开发预算不可读取")?).map_err(|_|"开发预算不合法".into())}else{Ok(json!({"attempts_reserved":0,"estimated_rmb":0.0,"input_tokens":0,"output_tokens":0}))}
}
fn totals(report:&mut Value,root:&Path)->Result<()> {
    let cases=report["results"].as_array().ok_or("测试结果格式不合法")?;
    let passed=cases.iter().filter(|v|v["passed"]==true).count();
    let input:u64=cases.iter().filter_map(|v|v["reply"]["meta"]["usage"]["input_tokens"].as_u64()).sum();
    let output:u64=cases.iter().filter_map(|v|v["reply"]["meta"]["usage"]["output_tokens"].as_u64()).sum();
    let after=provider_budget(root)?;
    report["summary"]=json!({"completed":cases.len(),"passed":passed,"failed":cases.len()-passed,"successful_response_input_tokens":input,"successful_response_output_tokens":output,
        "provider_attempts":after["attempts_reserved"].as_u64().unwrap_or(0).saturating_sub(report["budget_before"]["attempts_reserved"].as_u64().unwrap_or(0)),
        "provider_reported_input_tokens":after["input_tokens"].as_u64().unwrap_or(0).saturating_sub(report["budget_before"]["input_tokens"].as_u64().unwrap_or(0)),
        "provider_reported_output_tokens":after["output_tokens"].as_u64().unwrap_or(0).saturating_sub(report["budget_before"]["output_tokens"].as_u64().unwrap_or(0)),
        "estimated_rmb":after["estimated_rmb"].as_f64().unwrap_or(0.0)-report["budget_before"]["estimated_rmb"].as_f64().unwrap_or(0.0),"billing_verified":false,"matrix_events_sent":0});
    report["updated_at_local_collection_unix_seconds"]=json!(now());Ok(())
}

pub(crate) fn run(root:&Path,suite_path:&Path,report_path:&Path)->Result<()> {
    let bytes=std::fs::read(suite_path).map_err(|_|"合成测试文件不可读取")?;
    if bytes.len()>256*1024{return Err("测试文件超出范围".into());}
    let suite:Suite=serde_json::from_slice(&bytes).map_err(|_|"合成测试格式不合法")?;
    if suite.data_class!="synthetic"||suite.cases.is_empty()||suite.cases.len()>80{return Err("只支持显式标注 synthetic 的 1 至 80 项合成测试".into());}
    let mut ids=BTreeSet::new();
    for case in &suite.cases {if case.id.is_empty()||case.id.len()>64||!case.id.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-')||!ids.insert(&case.id){return Err("测试编号必须唯一且为短英文编号".into());}
        if let CaseKind::Preference{turns,expected}=&case.kind {model::preference_request(turns)?;if expected.ready!=expected.preferences.is_some(){return Err("完整测试必须给出确切期望时段和人数".into());}if let Some(p)=&expected.preferences{p.validate()?;}}
    }
    let hash=hex::encode(Sha256::digest(&bytes));
    let mut report=if report_path.is_file(){let value:Value=serde_json::from_slice(&std::fs::read(report_path).map_err(|_|"原测试记录不可读取")?).map_err(|_|"原测试记录不合法")?;
        if value["suite_sha256"]!=hash||value["version"]!=env!("CARGO_PKG_VERSION")||!value["results"].is_array(){return Err("原记录属于不同测试集或版本，请使用新的结果文件".into());}value
    }else{json!({"version":env!("CARGO_PKG_VERSION"),"model":"MiniMax-M3","official_model_host":true,"data_class":"synthetic","suite_sha256":hash,"limits":{"calls_per_day":80,"tokens_per_day":180000,"per_minute":3,"profile_reservation_rmb":10},"budget_before":provider_budget(root)?,"started_at_local_collection_unix_seconds":now(),"results":[]})};
    let host=model::evaluation_host(root)?;
    let mut previous_start:Option<Instant>=None;
    for case in suite.cases {
        let (request,activity)=match &case.kind {
            CaseKind::Preference{turns,..}=>(model::preference_request(turns)?,None),
            CaseKind::Explain{scenario}=>{let a=synthetic_activity(*scenario)?;(model::explain_request(&a),Some(a))},
            CaseKind::Note{scenario}=>{let a=synthetic_activity(*scenario)?;(model::note_request(&a),Some(a))},
        };
        let request_record=json!({"task":request.task,"input":request.input,"schema":request.schema});
        let request_hash=hex::encode(Sha256::digest(serde_json::to_vec(&request_record).unwrap()));
        if let Some(existing)=report["results"].as_array().unwrap().iter().find(|v|v["id"]==case.id){
            if existing["request_sha256"]!=request_hash{return Err("请求说明已变化，请保留原记录并使用新的结果文件".into());}continue;
        }
        if let Some(start)=previous_start {let elapsed=start.elapsed();let interval=Duration::from_millis(20500);if elapsed<interval{std::thread::sleep(interval-elapsed);}}
        previous_start=Some(Instant::now());
        let collected=now();let started=Instant::now();
        let completion=host.complete(model::EVALUATION_APP,request);
        let result=match completion {
            Ok(completion)=>{
                let reply=completion.to_reply();
                let parsed:Result<Value>=match &case.kind {
                    CaseKind::Preference{turns,expected}=>model::parse_preferences(&completion.output,turns).map(|draft|{
                        let p=draft.preferences().ok();let passed=(!draft.needs_clarification)==expected.ready&&p==expected.preferences;
                        json!({"passed":passed,"draft":draft,"typed_preferences":p,"expected":expected})}),
                    CaseKind::Explain{..}=>model::parse_explanation(&completion.output,activity.as_ref().unwrap()).map(|s|json!({"passed":true,"explanation":s})),
                    CaseKind::Note{..}=>model::parse_note(&completion.output).map(|n|json!({"passed":true,"draft_title":n.title,"draft_markdown":n.markdown,"publication_requires_review":true})),
                };
                match parsed {Ok(checked)=>json!({"id":case.id,"passed":checked["passed"],"request":request_record,"request_sha256":request_hash,"checked":checked,"reply":reply,"latency_ms":started.elapsed().as_millis(),"collected_at_unix_seconds":collected}),
                    Err(reason)=>json!({"id":case.id,"passed":false,"request":request_record,"request_sha256":request_hash,"reason":reason,"reply":reply,"latency_ms":started.elapsed().as_millis(),"collected_at_unix_seconds":collected})}
            },
            Err(refused)=>json!({"id":case.id,"passed":false,"request":request_record,"request_sha256":request_hash,"refusal":refused.code.as_str(),"collected_at_unix_seconds":collected}),
        };
        let stop=result["refusal"].as_str().is_some_and(|s|matches!(s,"budget"|"no_provider"));
        report["results"].as_array_mut().unwrap().push(result.clone());totals(&mut report,root)?;save_report(report_path,&report)?;
        println!("{}",json!({"case":case.id,"passed":result["passed"],"refusal":result["refusal"],"input_tokens":result["reply"]["meta"]["usage"]["input_tokens"],"output_tokens":result["reply"]["meta"]["usage"]["output_tokens"]}));
        if stop{return Err("模型预算或配置不可用，已停止；记录与原计数保留".into());}
    }
    totals(&mut report,root)?;report["completed"]=true.into();save_report(report_path,&report)?;println!("{}",report["summary"]);Ok(())
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn synthetic_scenarios_keep_valid_business_state_without_identifiers_in_requests() {
        for s in [Scenario::Empty,Scenario::Eligible,Scenario::Rejoined,Scenario::GroupBlocked,Scenario::TimeBlocked,Scenario::Paused,Scenario::Full,Scenario::Reserved,Scenario::Thirty] {
            let a=synthetic_activity(s).unwrap();a.validate().unwrap();let summary=model::anonymous_summary(&a).to_string();assert!(!summary.contains('@')&&!summary.contains('!')&&!summary.contains("合成参与者"));
        }
    }
    #[test] fn schema_supports_unknown_integer_fields_in_official_subset() {
        let request=model::preference_request(&["今晚有空".into()]).unwrap();
        let schema=octosense_llm_service::complete::schema::Schema::compile(&request.schema).unwrap();
        let output=json!({"earliest":null,"latest":null,"group":null,"needs_clarification":true,"questions":["几点到几点有空，几个人？"],"explanation":"时段和人数需要补充。"});
        schema.validate(&output).unwrap();let mut bad=output;bad["earliest"]=json!("19");assert!(schema.validate(&bad).is_err());
    }
    #[test] fn eval_job_is_strict_and_nested() {
        let value=json!({"data_class":"synthetic","cases":[{"id":"first","task":{"kind":"preference","turns":["今天19至21点，一人"],"expected":{"ready":true,"preferences":{"earliest":19,"latest":21,"group":1}}}}]});
        let suite:Suite=serde_json::from_value(value.clone()).unwrap();assert_eq!(suite.cases.len(),1);
        let mut bad=value;bad["cases"][0]["task"]["execute"]=true.into();assert!(serde_json::from_value::<Suite>(bad).is_err());
    }
}
