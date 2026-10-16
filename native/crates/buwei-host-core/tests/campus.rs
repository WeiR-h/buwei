use buwei_host_core::{*, campus::*, planning::*, discovery::*, participation::Intent};
use serde_json::json;

fn clock() -> u64 { calendar::parse("2026-10-16 12:00").unwrap() }
fn basics(kind: &str) -> ActivityBasics {
    let mut b = ActivityPlanDraft::from_campus_template("@owner:test", kind, clock()).unwrap().basics;
    b.start = "2026-10-20 19:00".into(); b.end = "2026-10-20 21:00".into();
    b.location = "虚构大学活动室".into(); b.capacity = 4;
    let c = b.campus.as_mut().unwrap();
    c.campus = "虚构大学东校区".into(); c.audience = "对主题感兴趣的同学".into();
    c.topic = "本人确认的校园主题".into();
    c.signup_deadline = "2026-10-19 18:00".into();
    if kind == "competition" { c.team_size = Some(2); }
    if kind == "music_festival" { c.role = Some(EntryRole::Audience); }
    b
}
fn activity(kind: &str) -> Activity {
    let b = basics(kind);
    let mut a = Activity::new("@owner:test".into(), "!campus:test".into(), b.title, b.capacity, 19, 21).unwrap();
    a.start = calendar::parse(&b.start).unwrap(); a.end = calendar::parse(&b.end).unwrap();
    a.metadata = Some(calendar::Metadata { activity_id: "a".repeat(32), template: b.template, location: b.location,
        description: b.description, campus: b.campus, archived: false, metrics: Default::default() });
    a.validate().unwrap(); a
}
fn prefs(a: &Activity, group: u8) -> Preferences { Preferences { earliest:a.start, latest:a.end, group } }
fn listing(a: &Activity) -> Listing {
    Listing { community:"!community:test".into(), event_id:"$announcement".into(), server_time:clock(), announcement:Announcement::from_activity(a,false).unwrap() }
}
fn update(field: &str, value: &str, source: &str) -> FieldUpdate {
    FieldUpdate { field:field.into(),value:value.into(),source:source.into() }
}

#[test]
fn campus_catalog_finds_the_four_requested_scenarios_and_club_recruitment() {
    for (label, id) in [("讲座","lecture"),("竞赛","competition"),("观影","movie"),("音乐节","music_festival"),("社团招新","club")] {
        assert_eq!(activity_types::parse(label).unwrap(), id);
        assert!(activity_types::search("校园").iter().any(|t|t.id==id));
        let d=ActivityPlanDraft::from_campus_template("@owner:test",id,clock()).unwrap();
        assert!(d.basics.campus.is_some()); assert!(!d.agenda.is_empty());
        assert!(d.activity_id.is_none() && d.setup_id.is_none());
        assert!(!playbook(id).checklist.is_empty());
    }
    assert!(ActivityPlanDraft::from_campus_template("@owner:test","dining",clock()).is_err());
}
#[test]
fn movie_keeps_its_existing_general_template_and_can_opt_into_campus() {
    let ordinary=ActivityPlanDraft::from_template("@owner:test","movie",clock()).unwrap();
    let campus=ActivityPlanDraft::from_campus_template("@owner:test","movie",clock()).unwrap();
    assert!(ordinary.basics.campus.is_none()); assert!(campus.basics.campus.is_some());
    assert_eq!(ordinary.basics.template,campus.basics.template);
}
#[test]
fn templates_do_not_invent_campus_speakers_deadlines_or_eligibility() {
    let d=ActivityPlanDraft::from_campus_template("@owner:test","lecture",clock()).unwrap();
    let c=d.basics.campus.unwrap();
    assert_eq!(c,CampusDetails::default());
    assert!(!c.summary("lecture").contains("已预约"));
    assert!(d.confirmed_fields.is_empty());
}
#[test]
fn competition_asks_for_team_size_and_counts_seats_as_people() {
    let mut b=basics("competition"); b.campus.as_mut().unwrap().team_size=None;
    assert_eq!(b.questions().len(),1); assert!(b.questions()[0].contains("个人赛"));
    assert!(b.ready(clock()).is_err());
    b.campus.as_mut().unwrap().team_size=Some(2); b.capacity=1;
    assert!(b.ready(clock()).is_err()); b.capacity=4; assert!(b.ready(clock()).is_ok());
    for size in [0,9,30] { b.campus.as_mut().unwrap().team_size=Some(size); assert!(b.validate_draft().is_err()); }
}
#[test]
fn deadline_is_independent_of_start_and_must_be_explicit_and_future_at_creation() {
    let mut b=basics("lecture");
    b.campus.as_mut().unwrap().signup_deadline="2026-10-21 12:00".into(); assert!(b.ready(clock()).is_err());
    b.campus.as_mut().unwrap().signup_deadline="2026-10-15 12:00".into();
    assert!(b.validate_draft().is_ok()); assert!(b.ready(clock()).is_err());
    b.campus.as_mut().unwrap().signup_deadline.clear(); assert!(b.ready(clock()).is_ok());
    b.campus.as_mut().unwrap().signup_deadline="明天".into(); assert!(b.validate_draft().is_err());
}
#[test]
fn model_campus_fields_can_precede_type_and_preserve_unmentioned_fields() {
    let mut d=ActivityPlanDraft::new("@owner:test","校园讲座",clock()).unwrap();
    let text="在虚构大学组织校园讲座，主题是城市观察，面向全校同学";
    d.merge(text,PlanProposal { updates:vec![
        update("campus.campus","虚构大学","虚构大学"),update("campus.topic","城市观察","主题是城市观察"),
        update("template","lecture","校园讲座"),update("campus.audience","全校同学","面向全校同学")], ..Default::default() },clock()).unwrap();
    assert_eq!(d.basics.campus.as_ref().unwrap().topic,"城市观察");
    let before=d.clone();
    d.merge("面向一年级同学",PlanProposal {updates:vec![update("campus.audience","一年级同学","面向一年级同学")],..Default::default()},clock()).unwrap();
    assert_eq!(d.basics.campus.as_ref().unwrap().topic,before.basics.campus.as_ref().unwrap().topic);
    assert!(d.basics.campus.as_ref().unwrap().guests.is_empty());
}
#[test]
fn unsupported_identity_fields_and_invented_campus_facts_are_rejected_atomically() {
    let mut d=ActivityPlanDraft::from_campus_template("@owner:test","lecture",clock()).unwrap(); let old=d.clone();
    for u in [update("campus.guests","虚构教授","举办讲座"),update("campus.student_id","12345","12345"),update("owner","@other:test","@other:test")] {
        assert!(d.merge(&u.source.clone(),PlanProposal {updates:vec![u],..Default::default()},clock()).is_err());
        assert_eq!(d,old);
    }
}
#[test]
fn changing_campus_rules_invalidates_previous_creation_confirmation() {
    let mut d=ActivityPlanDraft::from_campus_template("@owner:test","competition",clock()).unwrap();
    d.confirm_basics(basics("competition"),clock()).unwrap(); let digest=d.creation_digest(clock()).unwrap();
    d.merge("改成个人赛",PlanProposal {updates:vec![update("campus.team_size","1","个人赛")],..Default::default()},clock()).unwrap();
    assert!(d.creation_digest(clock()).is_err());
    d.confirm_basics(d.basics.clone(),clock()).unwrap(); assert_ne!(digest,d.creation_digest(clock()).unwrap());
}
#[test]
fn old_basics_and_activity_snapshots_read_without_campus_and_roundtrip_unchanged() {
    let mut a=activity("movie"); a.metadata.as_mut().unwrap().campus=None;
    let value=serde_json::to_value(&a).unwrap(); assert!(value["metadata"].get("campus").is_none());
    let restored:Activity=serde_json::from_value(value.clone()).unwrap(); restored.validate().unwrap();
    assert_eq!(serde_json::to_value(restored).unwrap(),value);
    let mut b=basics("movie"); b.campus=None;
    let v=serde_json::to_value(&b).unwrap(); assert!(v.get("campus").is_none());
    assert_eq!(serde_json::from_value::<ActivityBasics>(v).unwrap(),b);
}
#[test]
fn campus_snapshots_and_version_three_cards_preserve_public_rules() {
    let a=activity("competition"); let mut card=Announcement::from_activity(&a,false).unwrap();
    assert_eq!(card.protocol,3); card.validate().unwrap();
    assert_eq!(card.campus,a.metadata.as_ref().unwrap().campus);
    card.audience=Audience::Public; card.validate().unwrap();
    card.protocol=2; assert!(card.validate().is_err());
    card.protocol=3; card.campus=None; assert!(card.validate().is_err());
    let restored:Activity=serde_json::from_value(serde_json::to_value(&a).unwrap()).unwrap(); restored.validate().unwrap();
}
#[test]
fn campus_card_is_a_clue_and_does_not_grant_identity_or_change_queue() {
    let a=activity("lecture"); let l=listing(&a);
    let mut event=json!({"type":"m.room.message","room_id":l.community,"event_id":"$x","sender":a.owner,"origin_server_ts":clock()*1000,"content":{"org.buwei.community":l.announcement}});
    assert!(Listing::parse(&event,&l.community,clock()).is_ok());
    event["sender"]=json!("@forged:test"); assert!(Listing::parse(&event,&l.community,clock()).is_err());
    event["sender"]=json!(a.owner); event["content"]["org.buwei.community"]["campus"]["student_id"]=json!("12345");
    assert!(Listing::parse(&event,&l.community,clock()).is_err()); assert!(a.people.is_empty());
}
#[test]
fn campus_filter_keeps_scopes_and_obeys_the_deadline() {
    let a=activity("movie");let l=listing(&a);let f=Filter{template:"campus".into(),..Default::default()};
    f.validate().unwrap();assert!(f.matches(&l,clock()));
    let deadline=a.metadata.as_ref().unwrap().campus.as_ref().unwrap().deadline().unwrap().unwrap();
    assert!(!f.matches(&l,deadline));
    let mut ordinary=l.clone();ordinary.announcement.campus=None;ordinary.announcement.protocol=1;
    assert!(!f.matches(&ordinary,clock()));
    assert!(Filter{template:"movie".into(),..Default::default()}.matches(&ordinary,clock()));
}
#[test]
fn signup_deadline_is_enforced_in_preview_and_ui_with_exact_boundary() {
    let a=activity("lecture");let deadline=a.metadata.as_ref().unwrap().campus.as_ref().unwrap().deadline().unwrap().unwrap();
    let i=Intent::Join{preferences:prefs(&a,1)};
    assert!(i.validate(&a,"@member:test",deadline-1).is_ok());assert!(i.validate(&a,"@member:test",deadline).is_err());
    assert!(interaction::participant_controls(&a,"@member:test",deadline-1).register);
    assert!(!interaction::participant_controls(&a,"@member:test",deadline).register);
}
#[test]
fn wrong_team_size_is_blocked_even_without_the_ui_or_model() {
    let mut a=activity("competition");let p=prefs(&a,1);
    assert!(Intent::Join{preferences:p.clone()}.validate(&a,"@member:test",clock()).is_err());
    assert!(a.join_own("@member:test".into(),"成员".into(),p).is_err()); assert!(a.people.is_empty());
    let p=prefs(&a,2);a.join_own("@member:test".into(),"成员".into(),p).unwrap();a.validate().unwrap();
}
#[test]
fn quick_registration_uses_the_competition_rule_and_keeps_explicit_input_for_review() {
    let a=activity("competition");
    let (p,source)=interaction::whole_activity(&a,"@member:test",None).unwrap(); assert_eq!(p.group,2); assert!(source.contains("竞赛"));
    let (p,_)=interaction::explicit_whole("参加整场，1人",&a,"@member:test",None).unwrap().unwrap(); assert_eq!(p.group,1);
    assert!(Intent::Join{preferences:p}.validate(&a,"@member:test",clock()).is_err());
}
#[test]
fn waiting_team_updates_keep_position_and_a_team_never_splits_into_one_seat() {
    let mut a=activity("competition");a.capacity=3;
    for actor in ["@first:test","@next:test"] { let p=prefs(&a,2);a.join_own(actor.into(),"队伍".into(),p).unwrap(); }
    let joined=a.people[1].joined;let mut p=prefs(&a,2);p.latest+=3600;
    a.join_own("@next:test".into(),"队伍".into(),p).unwrap();assert_eq!(a.people[1].joined,joined);
    a.people[0].status=PersonStatus::Confirmed;a.validate().unwrap();assert_eq!(a.free(),1);assert!(a.candidate().is_none());
}
#[test]
fn deadline_does_not_erase_waiting_teams_or_prevent_cancellation() {
    let mut a=activity("competition");let p=prefs(&a,2);a.join_own("@member:test".into(),"队伍".into(),p.clone()).unwrap();
    let deadline=a.metadata.as_ref().unwrap().campus.as_ref().unwrap().deadline().unwrap().unwrap();
    assert!(Intent::Join{preferences:p}.validate(&a,"@member:test",deadline).is_err());assert_eq!(a.candidate(),Some("@member:test"));
    assert!(Intent::Cancel.validate(&a,"@member:test",deadline).is_ok());a.cancel_own("@member:test").unwrap();assert_eq!(a.free(),4);
}
#[test]
fn expired_entry_requests_cannot_use_the_community_auto_invitation_rule() {
    let a=activity("lecture");let l=listing(&a);let deadline=l.announcement.campus.as_ref().unwrap().deadline().unwrap().unwrap();
    let mut e=json!({"type":"org.buwei.entry.request","sender":"@member:test","event_id":"$entry","origin_server_ts":(deadline-1)*1000,"content":{"protocol":1,"announcement_event":l.event_id,"activity_id":l.announcement.activity_id,"activity_room":l.announcement.activity_room}});
    assert!(EntryRequest::parse(&e,&l.community,&l,deadline+60).is_ok());
    e["origin_server_ts"]=json!(deadline*1000);assert!(EntryRequest::parse(&e,&l.community,&l,deadline+60).is_err());
}
#[test]
fn campus_roster_export_adds_public_context_and_escapes_spreadsheet_formulas() {
    let mut a=activity("competition");let c=a.metadata.as_mut().unwrap().campus.as_mut().unwrap();
    c.campus=" =HYPERLINK(\"example\")".into();c.topic="+校园主题".into();
    let p=prefs(&a,2);a.join_own("@member:test".into(),"队伍".into(),p).unwrap();
    let csv=String::from_utf8(roster::csv(&a,&a.owner,clock(),clock()).unwrap()).unwrap();
    assert!(csv.starts_with('\u{feff}'));assert!(csv.contains("学校校区,面向人群,校园主题,报名截止北京时间,每份报名人数"));
    assert!(csv.contains("' =HYPERLINK") && csv.contains("'+校园主题"));assert!(csv.contains("\"2\""));
    assert!(roster::csv(&a,"@member:test",clock(),clock()).is_err());
    assert!(!csv.contains("student_id") && !csv.contains("preferences"));
}
#[test]
fn campus_answers_only_use_the_published_organizer_facts() {
    let a=activity("lecture");let answer=facts::render(&a,&["campus_details".into()]).unwrap();
    assert!(answer.contains("本人确认的校园主题") && answer.contains("依据：campus_details"));
    assert!(!answer.contains("教授") && !answer.contains("已获得学分"));
}
#[test]
fn copying_the_next_campus_session_keeps_context_but_clears_deadline() {
    let c=basics("competition").campus.unwrap();let next=c.for_next_event();
    assert!(next.signup_deadline.is_empty());assert_eq!(next.team_size,c.team_size);assert_eq!(next.topic,c.topic);
}
#[test]
fn music_festival_requires_a_reviewed_audience_or_performer_session() {
    let mut b=basics("music_festival");b.campus.as_mut().unwrap().role=None;
    assert!(b.questions().iter().any(|q|q.contains("观众")));assert!(b.ready(clock()).is_err());
    b.campus.as_mut().unwrap().role=Some(EntryRole::Audience);b.ready(clock()).unwrap();
    assert!(b.campus.as_ref().unwrap().summary("music_festival").contains("观众报名"));
    b.campus.as_mut().unwrap().role=Some(EntryRole::Performer);b.ready(clock()).unwrap();
    assert!(b.campus.as_ref().unwrap().summary("music_festival").contains("演出招募"));
    assert!(EntryRole::parse("自动报名所有人").is_err());
    let mut ordinary=basics("lecture");ordinary.campus.as_mut().unwrap().role=Some(EntryRole::Performer);
    assert!(ordinary.ready(clock()).is_err());
}
#[test]
fn a_music_festival_model_cannot_invent_the_participation_role() {
    let mut d=ActivityPlanDraft::from_campus_template("@owner:test","music_festival",clock()).unwrap();let old=d.clone();
    assert!(d.merge("举办音乐节",PlanProposal{updates:vec![update("campus.role","performer","音乐节")],..Default::default()},clock()).is_err());assert_eq!(d,old);
    d.merge("本场是观众报名",PlanProposal{updates:vec![update("campus.role","audience","观众报名")],..Default::default()},clock()).unwrap();
    assert_eq!(d.basics.campus.unwrap().role,Some(EntryRole::Audience));
}
#[test]
fn a_direct_preference_edit_cannot_bypass_competition_team_size() {
    let mut a=activity("competition");let p=prefs(&a,2);a.join_own("@member:test".into(),"队伍".into(),p).unwrap();
    let old=a.clone();let wrong=prefs(&a,1);assert!(a.confirm_preferences("@member:test",wrong).is_err());assert_eq!(serde_json::to_value(&a).unwrap(),serde_json::to_value(&old).unwrap());
}
#[test]
fn activity_selection_shows_campus_deadline_team_size_and_festival_role() {
    let competition=Announcement::from_activity(&activity("competition"),false).unwrap().listing_text();
    assert!(competition.contains("虚构大学") && competition.contains("每份 2 人") && competition.contains("报名截止 2026-10-19 18:00"));
    let mut a=activity("music_festival");let c=a.metadata.as_mut().unwrap().campus.as_mut().unwrap();
    let long_topic="很长的校园音乐节主题".repeat(8);c.topic=long_topic.clone();c.role=Some(EntryRole::Performer);
    let text=Announcement::from_activity(&a,false).unwrap().listing_text();
    assert!(text.contains("演出招募") && text.contains('…'));assert!(!text.contains(&long_topic));
    assert_eq!(EntryRole::parse("AUDIENCE").unwrap(),EntryRole::Audience);
}
