use action_receipts::Authority;
use buwei_host_core::{calendar, discovery::*};
use serde_json::{Value, json};
fn clock() -> u64 {
    calendar::parse("2026-10-12 12:00").unwrap()
}
fn event(id: &str) -> Value {
    json!({"event_id":id,"type":"m.room.message","sender":"@owner:test","origin_server_ts":clock()*1000,"content":{"org.buwei.community":{"protocol":1,"activity_id":"a".repeat(32),"activity_room":"!activity:test","organizer":"@owner:test","title":"读书会","template":"reading","location":"社区阅览室","start":clock()+86400,"end":clock()+93600,"capacity":2,"free":1,"revision":1,"withdrawn":false}}})
}
fn request(l: &Listing) -> Value {
    json!({"event_id":"$request","type":"org.buwei.entry.request","sender":"@member:test","origin_server_ts":(clock()+1)*1000,"content":{"protocol":1,"activity_id":l.announcement.activity_id,"activity_room":l.announcement.activity_room,"announcement_event":l.event_id}})
}
#[test]
fn cards_do_not_grant_membership_and_forged_identity_is_rejected() {
    let mut e = event("$card");
    assert!(Listing::parse(&e, "!community:test", clock()).is_ok());
    e["sender"] = "@evil:test".into();
    assert!(Listing::parse(&e, "!community:test", clock()).is_err());
    e["sender"] = "@owner:test".into();
    e["content"]["org.buwei.community"]["protocol"] = 9.into();
    assert!(Listing::parse(&e, "!community:test", clock()).is_err());
}

#[test]
fn observing_own_publication_does_not_skip_other_organizers_messages() {
    let path=std::env::temp_dir().join(format!("buwei-sparse-{}.db",action_receipts::new_id()));
    let mut s=DiscoveryStore::open(&path,"@member:test").unwrap();
    s.apply("!community:test",&[json!({"event_id":"$before"})],clock()).unwrap();
    let first=event("$earlier");let mut own=event("$own");
    own["content"]["org.buwei.community"]["activity_id"]="b".repeat(32).into();
    s.observe("!community:test",&[own.clone()],clock()).unwrap();
    assert_eq!(s.cursor("!community:test").unwrap().as_deref(),Some("$before"));
    s.apply("!community:test",&[first,own],clock()).unwrap();
    assert_eq!(s.list().unwrap().len(),2);
    drop(s);
    let s=DiscoveryStore::open(&path,"@member:test").unwrap();
    assert_eq!(s.cursor("!community:test").unwrap().as_deref(),Some("$own"));
    drop(s);let _=std::fs::remove_file(path);
}

#[test]
fn old_sync_checkpoint_replays_without_losing_existing_request_receipts() {
    let path=std::env::temp_dir().join(format!("buwei-cursor-upgrade-{}.db",action_receipts::new_id()));
    let mut s=DiscoveryStore::open(&path,"@member:test").unwrap();
    let l=Listing::parse(&event("$card"),"!community:test",clock()).unwrap();
    let events=vec![event("$card"),request(&l)];s.apply("!community:test",&events,clock()+2).unwrap();
    s.set_request("$request","unknown",Some("original-operation")).unwrap();drop(s);
    let db=rusqlite::Connection::open(&path).unwrap();db.execute("UPDATE cursor_versions SET version=1",[]).unwrap();drop(db);
    let mut s=DiscoveryStore::open(&path,"@member:test").unwrap();
    assert!(s.cursor("!community:test").unwrap().is_none());
    s.apply("!community:test",&events,clock()+2).unwrap();
    let rows=s.requests().unwrap();assert_eq!(rows.len(),1);assert_eq!(rows[0].1,"unknown");assert_eq!(rows[0].2.as_deref(),Some("original-operation"));
    drop(s);let _=std::fs::remove_file(path);
}
#[test]
fn request_binds_original_event_actor_and_activity() {
    let l = Listing::parse(&event("$card"), "!community:test", clock()).unwrap();
    let mut e = request(&l);
    assert!(EntryRequest::parse(&e, "!community:test", &l, clock() + 1).is_ok());
    e["content"]["activity_room"] = "!wrong:test".into();
    assert!(EntryRequest::parse(&e, "!community:test", &l, clock() + 1).is_err());
    e = request(&l);
    e["sender"] = "@owner:test".into();
    assert!(EntryRequest::parse(&e, "!community:test", &l, clock() + 1).is_err());
}
#[test]
fn member_filters_dates_and_templates_without_allocating_seats() {
    let l = Listing::parse(&event("$card"), "!community:test", clock()).unwrap();
    let f = Filter {
        template: "reading".into(),
        date: "2026-10-13".into(),
        available_only: true,
    };
    assert!(f.matches(&l, clock()));
    assert!(!f.matches(&l, l.announcement.start));
    let mut full = l.clone();
    full.announcement.free = 0;
    assert!(!f.matches(&full, clock()));
    assert!(Filter::default().matches(&full, clock()));
}
#[test]
fn policy_checks_membership_expiry_and_thirty_new_invitations() {
    let l = Listing::parse(&event("$card"), "!community:test", clock()).unwrap();
    let r = EntryRequest::parse(&request(&l), &l.community, &l, clock() + 1).unwrap();
    let a = Authority::default();
    a.set_account(Some("@owner:test"));
    let g = a
        .grant("buwei-community", &["admit_entry"], clock(), 3600)
        .unwrap();
    let mut p = AccessPolicy::issue(&g, &l, clock(), clock() + 3600).unwrap();
    assert!(p.check(&g, &r, true, clock() + 2).is_ok());
    assert!(p.check(&g, &r, false, clock() + 2).is_err());
    for n in 0..30 {
        p.used.insert(format!("${n}"));
    }
    assert!(p.check(&g, &r, true, clock() + 2).is_err());
    p.used.insert(r.event_id.clone());
    assert!(p.check(&g, &r, true, clock() + 2).is_ok());
    assert!(p.check(&g, &r, true, clock() + 3600).is_err());
    g.revoke();
    assert!(p.check(&g, &r, true, clock() + 2).is_err());
}
#[test]
fn discovery_isolates_accounts_and_deduplicates_events_across_restart() {
    let path =
        std::env::temp_dir().join(format!("buwei-discovery-{}.db", action_receipts::new_id()));
    let mut s = DiscoveryStore::open(&path, "@a:test").unwrap();
    let l = Listing::parse(&event("$card"), "!community:test", clock()).unwrap();
    let values = vec![event("$card"), request(&l)];
    s.apply("!community:test", &values, clock() + 2).unwrap();
    s.apply("!community:test", &values, clock() + 2).unwrap();
    assert_eq!(s.list().unwrap().len(), 1);
    assert_eq!(s.requests().unwrap().len(), 1);
    assert_eq!(
        s.cursor("!community:test").unwrap().as_deref(),
        Some("$request")
    );
    assert!(
        DiscoveryStore::open(&path, "@b:test")
            .unwrap()
            .list()
            .unwrap()
            .is_empty()
    );
    drop(s);
    let s = DiscoveryStore::open(&path, "@a:test").unwrap();
    assert_eq!(s.requests().unwrap().len(), 1);
    s.set_request("$request", "invited", Some("original-operation"))
        .unwrap();
    assert!(s.requests().unwrap().is_empty());
    drop(s);
    let _ = std::fs::remove_file(path);
}
