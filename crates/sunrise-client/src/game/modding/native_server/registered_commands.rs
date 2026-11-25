//! The mods' commands as the session holds them (`register_commands`). As on the reference
//! loader, the mods are asked whenever the session's command tree is built: when a world opens,
//! before its functions load, and on each `/reload`. The tree lives on `Game`, so a function
//! line naming a mod command parses with no hub; a hub, once there is one, takes that same tree.

use super::{CommandArguments, CommandExecutorSlots};
use mod_api::__private::{CommandExecutor, RetainedCallback};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};

pub(in crate::game) struct RegisteredExecutor {
    pub(in crate::game) callback: RetainedCallback,
    pub(in crate::game) call: Arc<CommandExecutor>,
}

/// The commands the mods registered, as the session's tree holds them.
pub(in crate::game) struct NativeCommands {
    /// Each root's traversal level.
    pub(in crate::game) levels: HashMap<String, u8>,
    /// Accepted display data, before filtering roots of a replacement tree.
    pub(in crate::game) tree: Value,
    pub(in crate::game) arguments: CommandArguments,
    /// Executable path to registration slot; never sent to the hub.
    pub(in crate::game) slots: CommandExecutorSlots,
    /// Process-local execution table indexed by the current dispatcher's parsed keys.
    pub(in crate::game) executors: Vec<RegisteredExecutor>,
}

/// The session's state of the mods' commands.
#[derive(Default)]
pub(in crate::game) struct NativeCommandState {
    /// A mod asked for its commands again (`refresh_commands`): done at the next server tick.
    pub(in crate::game) refresh: bool,
    pub(in crate::game) registered: Option<NativeCommands>,
    /// The display report of the currently installed server command tree.
    pub(in crate::game) report: Value,
    /// A hub holds the report revision it was given, not the dispatcher incarnation.
    pub(in crate::game) display_revision: u64,
}

pub(in crate::game) fn without_reserved_roots(
    report: &Value,
    reserved: impl Fn(&str) -> bool,
) -> Value {
    let mut report = report.clone();
    if let Some(roots) = report.get_mut("children").and_then(Value::as_object_mut) {
        roots.retain(|name, _| !reserved(name));
        if roots.is_empty() {
            return Value::Null;
        }
    }
    report
}
