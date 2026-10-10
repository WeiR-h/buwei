use buwei_host_core::{assistance::*, calendar};
fn at(s: &str) -> u64 {
    calendar::parse(s).unwrap()
}
fn weekly(days: Vec<u8>, begin: u16, finish: u16) -> AvailabilitySnapshot {
    AvailabilitySnapshot {
        weekdays: days,
        earliest_minute: begin,
        latest_minute: finish,
        confirmed_at: at("2026-10-09 12:00"),
    }
}
#[test]
fn weekday_and_whole_interval_are_required() {
    let s = weekly(vec![5, 6], 19 * 60, 22 * 60);
    assert!(s.covers(at("2026-10-10 19:30"), at("2026-10-10 21:00")));
    assert!(s.covers(at("2026-10-11 19:00"), at("2026-10-11 22:00")));
    assert!(!s.covers(at("2026-10-10 09:00"), at("2026-10-10 11:00")));
    assert!(!s.covers(at("2026-10-12 19:00"), at("2026-10-12 21:00")));
    assert!(!s.covers(at("2026-10-11 19:00"), at("2026-10-11 22:01")));
}
#[test]
fn overnight_belongs_to_the_window_start_day() {
    let s = weekly(vec![5], 23 * 60, 2 * 60);
    assert!(s.covers(at("2026-10-10 23:30"), at("2026-10-11 01:30")));
    assert!(s.covers(at("2026-10-11 00:30"), at("2026-10-11 02:00")));
    assert!(!s.covers(at("2026-10-11 23:30"), at("2026-10-12 01:30")));
    assert!(!s.covers(at("2026-10-10 23:30"), at("2026-10-11 02:01")));
}
#[test]
fn confirmed_snapshot_does_not_follow_later_preferences() {
    let mut p = PersonalPreferences {
        weekdays: vec![5],
        earliest_minute: Some(19 * 60),
        latest_minute: Some(22 * 60),
        confirmed_at: at("2026-10-09 12:00"),
        ..Default::default()
    };
    let s = AvailabilitySnapshot::from_preferences(&p).unwrap();
    p.weekdays = vec![0];
    p.earliest_minute = Some(8 * 60);
    assert!(s.covers(at("2026-10-10 20:00"), at("2026-10-10 21:00")));
    assert!(AvailabilitySnapshot::from_preferences(&PersonalPreferences::default()).is_err());
    assert!(!weekly(vec![8], 0, 1440).covers(at("2026-10-10 20:00"), at("2026-10-10 21:00")));
}
