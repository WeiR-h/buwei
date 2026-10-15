use buwei_host_core::performance::{CommandKind as Kind, CommandMetrics, WINDOW_PER_KIND};
use std::time::Duration;

#[test]
fn reports_real_nearest_rank_quantiles_and_keeps_failures_in_the_population() {
    let mut metrics = CommandMetrics::default();
    for n in 1..=100 { metrics.record(Kind::Sync, n % 4 != 0, Duration::from_millis(n)); }
    let report = metrics.report();
    assert_eq!(report.observations, 100);
    let sample = &report.commands[0];
    assert_eq!((sample.successes, sample.failures, sample.retained_samples), (75, 25, 100));
    assert_eq!((sample.p50_ms, sample.p95_ms, sample.maximum_ms), (50.0, 95.0, 100.0));
}

#[test]
fn memory_is_bounded_and_the_oldest_samples_leave_the_quantiles() {
    let mut metrics = CommandMetrics::default();
    metrics.record(Kind::Model, false, Duration::from_secs(900));
    for _ in 0..WINDOW_PER_KIND { metrics.record(Kind::Model, true, Duration::from_micros(1500)); }
    let report = metrics.report();
    assert_eq!(report.observations, WINDOW_PER_KIND as u64 + 1);
    let sample = &report.commands[0];
    assert_eq!(sample.retained_samples, WINDOW_PER_KIND);
    assert_eq!((sample.failures, sample.p95_ms, sample.maximum_ms), (0, 1.5, 1.5));
}

#[test]
fn identity_reset_discards_previous_samples_and_empty_reports_are_valid() {
    let mut metrics = CommandMetrics::default();
    assert!(metrics.report().commands.is_empty());
    metrics.record(Kind::Planning, true, Duration::ZERO);
    assert_eq!(metrics.report().commands[0].p95_ms, 0.0);
    metrics.clear();
    assert_eq!(metrics.report().observations, 0);
    assert!(metrics.report().commands.is_empty());
    metrics.record(Kind::Read, false, Duration::from_millis(23));
    let json = serde_json::to_value(metrics.report()).unwrap();
    assert_eq!(json["commands"][0]["kind"], "read");
    assert_eq!(json["commands"][0]["failures"], 1);
    assert_eq!(json.as_object().unwrap().len(), 6);
}
