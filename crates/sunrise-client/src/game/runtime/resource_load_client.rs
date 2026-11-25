use super::*;
use crate::game::resource_load::Progress;
use crate::game::resource_load::STAGE_LINES;
use crate::game::resource_load::Step;
use crate::game::resource_load::named_scope;

impl Progress {
    pub(in crate::game) fn fraction(&self) -> f32 {
        self.value.load(Ordering::Relaxed) as f32 / (self.lines.len() - 1) as f32
    }
    pub(in crate::game) fn line(&self) -> &'static str {
        self.lines[(self.value.load(Ordering::Relaxed) as usize).min(self.lines.len() - 1)]
    }
    pub(in crate::game) fn new() -> Self {
        Self::with(&STAGE_LINES)
    }
    pub(in crate::game) fn set(&self, stage: u32) {
        self.value.store(stage, Ordering::Relaxed);
    }
    pub(in crate::game) fn with(lines: &'static [&'static str]) -> Self {
        Self {
            value: Arc::new(AtomicU32::new(0)),
            lines,
        }
    }
}

impl<'scope, T: Send + 'scope> Step<'scope, T> {
    pub(in crate::game) fn join(self) -> T {
        match self {
            Self::Thread(handle) => handle.join().unwrap(),
            Self::Later(f) => f(),
        }
    }
    /// `name` names the thread when one is spawned ([`named_scope`]).
    pub(in crate::game) fn spawn(
        scope: &'scope std::thread::Scope<'scope, '_>,
        parallel: bool,
        name: &str,
        f: impl FnOnce() -> T + Send + 'scope,
    ) -> Self {
        if parallel {
            Self::Thread(named_scope(scope, name, f))
        } else {
            Self::Later(Box::new(f))
        }
    }
}
