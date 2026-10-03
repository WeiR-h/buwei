//! Host-owned cancellation remains observable while a worker reads the network.
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
#[derive(Clone, Default)]
pub(crate) struct Interlock(Arc<Mutex<(u64, BTreeMap<String, u64>)>>);
#[derive(Clone)]
pub(crate) struct Permit {
    host: Interlock,
    activity: String,
    generation: (u64, u64),
}
impl Interlock {
    pub(crate) fn issue(&self, activity: &str) -> Permit {
        let state = self.0.lock().unwrap();
        Permit {
            host: self.clone(),
            activity: activity.into(),
            generation: (state.0, *state.1.get(activity).unwrap_or(&0)),
        }
    }
    pub(crate) fn invalidate(&self, activity: &str) {
        *self.0.lock().unwrap().1.entry(activity.into()).or_default() += 1;
    }
    pub(crate) fn invalidate_all(&self) {
        self.0.lock().unwrap().0 += 1;
    }
}
impl Permit {
    pub(crate) fn valid(&self) -> bool {
        let state = self.host.0.lock().unwrap();
        self.generation == (state.0, *state.1.get(&self.activity).unwrap_or(&0))
    }
    /// Linearize cancellation with the first SDK poll. The callback must only
    /// hand off an async request; it must not wait for network completion.
    pub(crate) fn submit<T>(&self, handoff: impl FnOnce() -> T) -> Option<T> {
        let state = self.host.0.lock().ok()?;
        if self.generation != (state.0, *state.1.get(&self.activity).unwrap_or(&0)) {
            return None;
        }
        Some(handoff())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edits_during_a_network_read_cancel_only_that_activity() {
        let host = Interlock::default();
        let first = host.issue("first");
        let second = host.issue("second");
        let worker = first.clone();
        host.invalidate("first");
        assert!(!worker.valid());
        assert!(second.valid());
        assert!(host.issue("first").valid());
    }
    #[test]
    fn revoke_or_shutdown_invalidates_every_existing_permit() {
        let host = Interlock::default();
        let first = host.issue("first");
        let second = host.issue("second");
        host.invalidate_all();
        assert!(!first.valid());
        assert!(!second.valid());
        assert!(host.issue("first").valid());
    }
    #[test]
    fn revoked_permit_never_hands_a_request_to_the_sdk() {
        let host = Interlock::default();
        let permit = host.issue("first");
        let mut submissions = 0;
        host.invalidate_all();
        assert!(permit.submit(|| submissions += 1).is_none());
        assert_eq!(submissions, 0);
    }
    #[test]
    fn revoke_and_sdk_handoff_have_one_order() {
        let host = Interlock::default();
        let permit = host.issue("first");
        let revoke_host = host.clone();
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (revoked_tx, revoked_rx) = std::sync::mpsc::channel();
        let revoke = std::thread::spawn(move || {
            entered_rx.recv().unwrap();
            revoke_host.invalidate_all();
            revoked_tx.send(()).unwrap();
        });
        assert_eq!(
            permit.submit(|| {
                entered_tx.send(()).unwrap();
                assert!(revoked_rx.try_recv().is_err());
                "handed to SDK"
            }),
            Some("handed to SDK")
        );
        revoke.join().unwrap();
        revoked_rx.recv().unwrap();
        assert!(permit.submit(|| "second request").is_none());
    }
}
