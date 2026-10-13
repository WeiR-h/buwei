use buwei_host_core::{
    assistance::PersonalPreferences,
    discovery::{Announcement, Audience, Listing},
    *,
};
use serde_json::{Value, json};
fn activity(owner: &str, room: &str) -> Activity {
    let mut a = Activity::new(owner.into(), room.into(), "读书会".into(), 2, 19, 21).unwrap();
    a.start = calendar::parse("2026-10-16 19:30").unwrap();
    a.end = a.start + 7200;
    a.metadata = Some(calendar::Metadata {
        activity_id: action_receipts::new_id(),
        template: "reading".into(),
        location: "阅览室".into(),
        description: String::new(),
        archived: false,
        metrics: Default::default(),
    });
    a
}
fn states() -> Vec<Value> {
    vec![
        json!({"type":"m.room.create","state_key":"","sender":"@directory:test","content":{}}),
        json!({"type":"m.room.join_rules","state_key":"","content":{"join_rule":"public"}}),
        json!({"type":"org.buwei.directory","state_key":"","sender":"@directory:test","content":{"protocol":1,"kind":"public","creator":"@directory:test"}}),
    ]
}
#[test]
fn roles_follow_each_activity_and_members_can_organize_their_own() {
    let a = activity("@a:test", "!a:test");
    let b = activity("@b:test", "!b:test");
    assert!(interaction::is_organizer(&a, "@a:test"));
    assert!(!interaction::is_organizer(&b, "@a:test"));
    assert!(interaction::is_organizer(&b, "@b:test"));
    assert!(!interaction::is_organizer(&a, "@b:test"));
    let before = serde_json::to_value((&a, &b)).unwrap();
    assert!(interaction::whole_activity(&a, "@a:test", None).is_err());
    assert_eq!(
        interaction::whole_activity(&b, "@a:test", None)
            .unwrap()
            .0
            .group,
        1
    );
    assert_eq!(serde_json::to_value((&a, &b)).unwrap(), before);
}
#[test]
fn full_activity_preview_uses_only_confirmed_defaults_without_changing_them() {
    let a = activity("@a:test", "!a:test");
    let mut preferences = PersonalPreferences {
        group: Some(2),
        ..Default::default()
    };
    assert_eq!(
        interaction::whole_activity(&a, "@b:test", Some(&preferences))
            .unwrap()
            .0
            .group,
        1
    );
    preferences.confirmed_at = 1;
    let before = preferences.clone();
    let (p, _) = interaction::whole_activity(&a, "@b:test", Some(&preferences)).unwrap();
    assert_eq!((p.earliest, p.latest, p.group), (a.start, a.end, 2));
    assert_eq!(preferences, before);
    assert_eq!(a.confirmed(), 0);
    assert_eq!(a.held(), 0);
}
#[test]
fn public_requires_server_public_rules_and_the_original_creator_marker() {
    let original = states();
    assert!(public_directory::verify(&original).is_ok());
    for mutation in 0..5 {
        let mut s = original.clone();
        match mutation {
            0 => s[1]["content"]["join_rule"] = "invite".into(),
            1 => s[2]["sender"] = "@evil:test".into(),
            2 => s[2]["content"]["creator"] = "@evil:test".into(),
            3 => s[2]["content"]["protocol"] = 99.into(),
            _ => s.push(json!({"type":"m.room.encryption","state_key":"","content":{}})),
        }
        assert!(public_directory::verify(&s).is_err());
    }
}
#[test]
fn old_community_cards_remain_compatible_and_public_cards_are_distinct() {
    let a = activity("@a:test", "!a:test");
    let old = Announcement::from_activity(&a, false).unwrap();
    let value = serde_json::to_value(&old).unwrap();
    assert!(value.get("audience").is_none());
    assert_eq!(
        serde_json::from_value::<Announcement>(value)
            .unwrap()
            .audience,
        Audience::Community
    );
    let mut public = old.clone();
    public.protocol = 2;
    public.audience = Audience::Public;
    assert!(public.validate().is_ok());
    public.protocol = 1;
    assert!(public.validate().is_err());
    public.protocol = 2;
    let event = json!({"type":"m.room.message","event_id":"$x","sender":a.owner,"room_id":"!public:test","origin_server_ts":1000,"content":{"org.buwei.community":public}});
    assert_eq!(
        Listing::parse(&event, "!public:test", 2)
            .unwrap()
            .announcement
            .audience,
        Audience::Public
    );
    assert!(Listing::parse(&event, "!community:test", 2).is_err());
}
