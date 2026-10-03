use action_receipts::new_id;
use buwei_host_core::model_budget::Budget;
#[test]
fn all_profiles_share_a_durable_ceiling() {
    let path = std::env::temp_dir().join(format!("shared-{}.db", new_id()));
    let mut a = Budget::open(&path).unwrap();
    a.configure(250000).unwrap();
    let x = a.reserve().unwrap();
    let mut b = Budget::open(&path).unwrap();
    b.reserve().unwrap();
    assert!(a.reserve().is_err());
    b.record(&x, 1000, 1000).unwrap();
    a.reserve().unwrap();
    assert!(b.record(&x, 1000, 1000).is_err());
    assert_eq!(a.summary().unwrap()["attempts"], 3);
}
#[test]
fn lost_responses_keep_their_reservation() {
    let path = std::env::temp_dir().join(format!("shared-{}.db", new_id()));
    let mut a = Budget::open(&path).unwrap();
    a.configure(100000).unwrap();
    a.reserve().unwrap();
    drop(a);
    let mut b = Budget::open(&path).unwrap();
    assert!(b.reserve().is_err());
    assert_eq!(b.summary().unwrap()["estimated_or_reserved_rmb"], 0.1);
}
#[test]
fn history_is_retained_when_the_user_sets_a_new_remaining_budget() {
    let mut a = Budget::open(":memory:").unwrap();
    a.import_history("organizer", 504900).unwrap();
    a.import_history("organizer", 504900).unwrap();
    a.configure(80000000).unwrap();
    assert_eq!(a.summary().unwrap()["history_estimated_rmb"], 0.5049);
    assert_eq!(a.summary().unwrap()["ceiling_rmb"], 80.0);
}
#[test]
fn imported_history_counts_toward_the_shared_limit() {
    let mut budget = Budget::open(":memory:").unwrap();
    budget.import_history("old-profile", 150000).unwrap();
    assert!(budget.configure(100000).is_err());
    budget.configure(250000).unwrap();
    budget.reserve().unwrap();
    assert!(budget.reserve().is_err());
}
#[test]
fn bounded_code_reviews_fit_one_reservation() {
    let mut budget = Budget::open(":memory:").unwrap();
    budget.configure(100000).unwrap();
    let id = budget.reserve().unwrap();
    assert!(budget.record(&id, 16385, 7168).is_err());
    assert!(budget.record(&id, 16384, 7169).is_err());
    budget.record(&id, 16384, 7168).unwrap();
    assert!(
        budget.summary().unwrap()["estimated_or_reserved_rmb"]
            .as_f64()
            .unwrap()
            <= 0.1
    );
}
