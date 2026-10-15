//! Bounded, content-free timings measured by the host's monotonic clock.
//!
//! These are command completion times, not frame rate or server-only latency.
//! The owning host discards them when its SDK identity changes. Nothing here
//! accepts an account, room, request, prompt, path, message or model output.
use serde::Serialize;
use std::{collections::{BTreeMap, VecDeque}, time::Duration};

pub const WINDOW_PER_KIND: usize = 128;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandKind {
    Planning,
    Discovery,
    Roster,
    Assistance,
    Navigation,
    Read,
    Authorization,
    Sync,
    Invitation,
    Participation,
    Article,
    Model,
    Setup,
}

#[derive(Clone, Copy)]
struct Sample { microseconds: u64, success: bool }

#[derive(Default)]
pub struct CommandMetrics {
    observations: u64,
    samples: BTreeMap<CommandKind, VecDeque<Sample>>,
}

#[derive(Debug, Serialize)]
pub struct Summary {
    pub kind: CommandKind,
    pub retained_samples: usize,
    pub successes: usize,
    pub failures: usize,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub maximum_ms: f64,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub schema: u8,
    pub observations: u64,
    pub window_per_kind: usize,
    pub clock: &'static str,
    pub scope: &'static str,
    pub commands: Vec<Summary>,
}

impl CommandMetrics {
    pub fn record(&mut self, kind: CommandKind, success: bool, elapsed: Duration) {
        self.observations = self.observations.saturating_add(1);
        let samples = self.samples.entry(kind).or_default();
        if samples.len() == WINDOW_PER_KIND { samples.pop_front(); }
        samples.push_back(Sample {
            microseconds: elapsed.as_micros().min(u64::MAX as u128) as u64,
            success,
        });
    }

    pub fn clear(&mut self) { *self = Self::default(); }

    pub fn report(&self) -> Report {
        let commands = self.samples.iter().map(|(&kind, samples)| {
            let mut times: Vec<_> = samples.iter().map(|s| s.microseconds).collect();
            times.sort_unstable();
            let quantile = |percent: usize| times[(times.len() * percent).div_ceil(100) - 1] as f64 / 1000.0;
            let successes = samples.iter().filter(|s| s.success).count();
            Summary {
                kind, retained_samples: samples.len(), successes,
                failures: samples.len() - successes,
                p50_ms: quantile(50), p95_ms: quantile(95),
                maximum_ms: *times.last().expect("recorded sample") as f64 / 1000.0,
            }
        }).collect();
        Report {
            schema: 1, observations: self.observations, window_per_kind: WINDOW_PER_KIND,
            clock: "monotonic", scope: "host command completion; latest samples per kind in the current identity session",
            commands,
        }
    }
}
