/// The roots of a player's outgoing command tree, after permission checks and
/// before any listener selections are applied. Root names are borrowed in send order.
// no wire: borrows the outgoing server command tree for this send.
#[derive(Clone, Copy)]
pub struct CommandTreeFilter<'a> {
    len: usize,
    name_at: &'a dyn Fn(usize) -> &'a str,
}

/// A listener's requested exclusions from an outgoing command tree.
// no wire: returned directly by an in-process command tree callback.
pub struct CommandTreeSelection(Vec<String>);

impl<'a> CommandTreeFilter<'a> {
    /// The permission-filtered root names, in the order the client would receive them.
    pub fn root_names(&self) -> impl ExactSizeIterator<Item = &'a str> {
        (0..self.len).map(self.name_at)
    }

    /// Select the roots to hide this send. Return the selection from the hook
    /// to apply it. Other listeners may hide more roots, but cannot restore any.
    pub fn hide_where(&self, mut hide: impl FnMut(&str) -> bool) -> Option<CommandTreeSelection> {
        let names: Vec<_> = self
            .root_names()
            .filter(|name| hide(name))
            .map(str::to_owned)
            .collect();
        (!names.is_empty()).then_some(CommandTreeSelection(names))
    }
}

/// Engine adapter; mods receive a view from the hook and do not construct one.
pub fn command_tree_filter<'a>(
    len: usize,
    name_at: &'a dyn Fn(usize) -> &'a str,
) -> CommandTreeFilter<'a> {
    CommandTreeFilter { len, name_at }
}

/// Return the selection to the engine without exposing its storage to mods.
pub fn selected_command_roots(selection: CommandTreeSelection) -> Vec<String> {
    selection.0
}
