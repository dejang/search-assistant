//! Live progress for a research run, as the webview sees it.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::Serialize;

type Listener = dyn Fn(&Activity<'_>) + Send + Sync;

/// Which agent an activity line belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Researcher {
    Lead,
    Subagent,
}

/// What the calling tool is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ResearchAction {
    Searching,
    Reading,
    Delegating,
}

/// One activity line, carrying the running totals the webview shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity<'a> {
    pub by: Researcher,
    pub action: ResearchAction,
    pub detail: &'a str,
    pub searches: usize,
    pub reads: usize,
    pub subagents: usize,
}

/// Totals for one run, counted and read out under one lock so the numbers a
/// viewer sees only ever climb, however subagents overlap.
#[derive(Default)]
struct Counts {
    searches: usize,
    reads: usize,
    subagents: usize,
}

/// Counts the work the research agents do and announces each call as it starts.
pub struct ResearchProgress {
    running: AtomicBool,
    run: AtomicU64,
    counts: Mutex<Counts>,
    listener: Option<Box<Listener>>,
}

impl Default for ResearchProgress {
    fn default() -> Self {
        Self::new()
    }
}

impl ResearchProgress {
    #[must_use]
    pub fn new() -> Self {
        Self {
            running: AtomicBool::new(false),
            run: AtomicU64::new(0),
            counts: Mutex::new(Counts::default()),
            listener: None,
        }
    }

    #[must_use]
    pub fn on_activity(mut self, listener: impl Fn(&Activity<'_>) + Send + Sync + 'static) -> Self {
        self.listener = Some(Box::new(listener));
        self
    }

    /// Takes the one run slot and zeroes the totals, or `None` if a question is
    /// already being answered. One model answers one question at a time, and a
    /// second run would report its counts over the first one's.
    pub fn begin(&self) -> Option<RunGuard<'_>> {
        self.running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .ok()
            .map(|_| {
                self.run.fetch_add(1, Ordering::SeqCst);
                *self.counts() = Counts::default();
                RunGuard { progress: self }
            })
    }

    /// The id of the run in flight, so lines the agents write while it runs can
    /// be read back as one run. Zero before the first `begin`.
    #[must_use]
    pub fn run_id(&self) -> u64 {
        self.run.load(Ordering::SeqCst)
    }

    /// One web search for `query`.
    pub fn searched(&self, by: Researcher, query: &str) {
        self.counted(by, ResearchAction::Searching, query);
    }

    /// One page read at `url`.
    pub fn read(&self, by: Researcher, url: &str) {
        self.counted(by, ResearchAction::Reading, url);
    }

    /// One subagent spawned on `sub_question`.
    pub fn delegated(&self, by: Researcher, sub_question: &str) {
        self.counted(by, ResearchAction::Delegating, sub_question);
    }

    fn counts(&self) -> MutexGuard<'_, Counts> {
        self.counts.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Bumps the tally this action belongs to and announces it inside the same
    /// lock, so no viewer is ever shown a total older than one it saw last.
    fn counted(&self, by: Researcher, action: ResearchAction, detail: &str) {
        let mut counts = self.counts();
        match action {
            ResearchAction::Searching => counts.searches += 1,
            ResearchAction::Reading => counts.reads += 1,
            ResearchAction::Delegating => counts.subagents += 1,
        }
        let activity = Activity {
            by,
            action,
            detail,
            searches: counts.searches,
            reads: counts.reads,
            subagents: counts.subagents,
        };
        if let Some(listener) = &self.listener {
            listener(&activity);
        }
    }
}

/// Frees the run slot, however the run ended.
pub struct RunGuard<'a> {
    progress: &'a ResearchProgress,
}

impl Drop for RunGuard<'_> {
    fn drop(&mut self) {
        self.progress.running.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::{Activity, ResearchAction, ResearchProgress, Researcher};

    #[test]
    fn begin_locks_out_a_second_run_until_the_guard_drops() {
        let progress = ResearchProgress::new();
        {
            let _first = progress.begin().expect("first run acquires the slot");
            assert!(progress.begin().is_none(), "second run must be rejected");
        }
        assert!(
            progress.begin().is_some(),
            "dropping the guard frees the slot"
        );
    }

    #[test]
    fn activity_counts_climb_within_a_run_and_reset_on_the_next_begin() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        let progress = ResearchProgress::new().on_activity(move |activity| {
            sink.lock().expect("lock").push((
                activity.searches,
                activity.reads,
                activity.subagents,
            ));
        });

        let run1 = progress.begin().expect("run 1");
        progress.delegated(Researcher::Lead, "angle 1");
        progress.searched(Researcher::Subagent, "rust async");
        progress.read(Researcher::Subagent, "https://example.com");
        drop(run1);

        let _run2 = progress.begin().expect("run 2");
        progress.searched(Researcher::Lead, "fresh query");

        let recorded = seen.lock().expect("lock").clone();
        assert_eq!(recorded, vec![(0, 0, 1), (1, 0, 1), (1, 1, 1), (1, 0, 0)]);
    }

    #[test]
    fn activity_serializes_as_the_webview_reads_it() {
        let activity = Activity {
            by: Researcher::Subagent,
            action: ResearchAction::Searching,
            detail: "tokio runtime",
            searches: 2,
            reads: 1,
            subagents: 1,
        };
        let value = serde_json::to_value(activity).expect("json");
        assert_eq!(value["by"], "subagent");
        assert_eq!(value["action"], "searching");
        assert_eq!(value["detail"], "tokio runtime");
        assert_eq!(value["searches"], 2);
        assert_eq!(value["reads"], 1);
        assert_eq!(value["subagents"], 1);
    }
}
