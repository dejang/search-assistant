//! Live progress for a research run, as the webview sees it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// Which agent an activity line belongs to.
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Researcher {
    Lead,
    Subagent,
}

/// What the calling tool is doing.
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ResearchAction {
    Searching,
    Reading,
    Delegating,
}

/// One activity line, carrying the running totals the webview shows.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Activity<'a> {
    by: Researcher,
    action: ResearchAction,
    detail: &'a str,
    searches: usize,
    reads: usize,
    subagents: usize,
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
    app: AppHandle,
    running: AtomicBool,
    counts: Mutex<Counts>,
}

impl ResearchProgress {
    #[must_use]
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            running: AtomicBool::new(false),
            counts: Mutex::new(Counts::default()),
        }
    }

    /// Takes the one run slot and zeroes the totals, or `None` if a question is
    /// already being answered. One model answers one question at a time, and a
    /// second run would report its counts over the first one's.
    pub fn begin(&self) -> Option<RunGuard<'_>> {
        self.running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .ok()
            .map(|_| {
                *self.counts() = Counts::default();
                RunGuard { progress: self }
            })
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
        let _ = self.app.emit("research://activity", &activity);
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
