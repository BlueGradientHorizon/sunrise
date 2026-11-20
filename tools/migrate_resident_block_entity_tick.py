#!/usr/bin/env python3
"""Remove the legacy block-entity tick reply plumbing for #3635 item 1.

Run once from the pre-conversion tree. The resident editor and its tests are
separate hand-written changes. Keep detached menu reply/writeback intact.
"""
from pathlib import Path
import json
import re

CLIENT = Path('crates/sunrise-client/src/game')
MODDING = CLIENT / 'modding'
VOCAB = Path('crates/sunrise-mod-vocab/src')
GEN = Path('crates/sunrise-mod-api-gen')


def rewrite(path, transform):
    old = path.read_text()
    new = transform(old)
    if old == new:
        raise ValueError(f'no migration in {path}')
    path.write_text(new)


def cut(text, start, end, replacement=''):
    i = text.index(start)
    j = text.index(end, i + len(start))
    return text[:i] + replacement + text[j:]


rewrite(CLIENT.with_suffix('.rs'), lambda s: ''.join(
    line for line in s.splitlines(keepends=True) if not re.search(r'block_entity_tick_(key|items):', line)))
rewrite(CLIENT / 'runtime/transactions.rs', lambda s: re.sub(
    r'        if let Some\((scope|key)\) = &mut self\.block_entity_tick_(items|key) \{\n[^\n]*\n        \}\n', '', s))
rewrite(CLIENT / 'runtime/transactions/item_entity.rs', lambda s: cut(cut(s,
    '    pub(super) fn abort_tick_item_transactions(', '    /// Sets item entity'),
    '        if self.block_entity_tick_key.is_some()', '        let Some(level)'))
rewrite(MODDING / 'dropped_item_setter.rs', lambda s: cut(s,
    '        if self.block_entity_tick_key.is_some()', '        let Some(i)'))
rewrite(MODDING / 'native_entity_remove.rs', lambda s: cut(s,
    '            Some(Target::Item(id)) if self.block_entity_tick_key.is_some()',
    '            Some(t) if self.journal_removal('))

# Put the unscoped implementations back at their original callers. Keep the
# general-entity tests; they have nothing to do with tick replies.
p = MODDING / 'native_entity_refs/tick_item_read.rs'
s = p.read_text()
target = s[s.index('    pub(super) fn unscoped_native_entity_target'):s.index('    #[cold]')].replace('unscoped_', '')
tests = s[s.index('#[cfg(test)]'):]
p = MODDING / 'native_entity_refs/tick_item_selector.rs'
s = p.read_text()
select = s[s.index('    pub(super) fn unscoped_level_all_entities'):s.index('    #[cold]')].replace('unscoped_', '')
rewrite(MODDING / 'native_entity_refs.rs', lambda s: cut(cut(s,
    '    pub(super) fn native_entity_target(', '    /// What a handle names on a client', target),
    '    pub(super) fn level_all_entities(', '    /// Entities whose bounding boxes', select)
    .replace('include!("native_entity_refs/tick_item_read.rs");', tests)
    .replace('include!("native_entity_refs/tick_item_selector.rs");\n', ''))
s = (MODDING / 'native_item_places/tick_item_read.rs').read_text()
body = s[s.index('        if !self.places_simulated()'):s.index('    #[cold]')]
rewrite(MODDING / 'native_item_places.rs', lambda s: cut(s,
    '        if self.block_entity_tick_items.is_some()', '    /// Component-only write,', body)
    .replace('include!("native_item_places/tick_item_read.rs");\n', ''))
rewrite(MODDING / 'native_item_entity_refs.rs', lambda s: cut(s,
    '        if self.block_entity_tick_items.is_some()', '    /// Writes a nonempty',
    '        Some(self.dropped.get(self.native_item_entity_index(item)?)?.stack.clone())\n    }\n')
    .replace('include!("native_item_entity_refs/tick_item_read.rs");\n', ''))

rewrite(VOCAB / 'block/types.rs', lambda s: cut(s,
    '/// What a block entity tick writes back.', 'impl std::ops::Deref for ModBlockEntityView',
    '''// no wire: borrowed payload, scoped to the owning call.
/// A menu hook's mutable detached payload, with a read-only type identity.
// exhaustive: only the game constructs this checked view.
pub struct ModBlockEntityView<'a> {
    pub(crate) entity: &'a mut ModBlockEntity,
}

impl<'a> ModBlockEntityView<'a> {
    #[doc(hidden)]
    pub fn new(entity: &'a mut ModBlockEntity) -> Self { Self { entity } }

    /// The block entity's immutable type id.
    pub fn id(&self) -> &str { self.entity.id() }

    /// Queue one-shot fields in the block entity's next update to its viewers.
    pub fn set_client_update_fields(&mut self, fields: Compound) {
        self.entity.client_update_fields = Some(fields);
    }
}

''').replace('        self.checkpoint_tick_listener();\n', ''))
rewrite(GEN / 'src/dispatch.rs', lambda s: cut(cut(cut(s,
    '        let write_back = if name == "block_entity_tick"', '        // Each listener gets'),
    '        let build = if name == "block_entity_tick"', '        // A trait mod'),
    '                _ if name == "block_entity_tick"', '                Contract::Cancel =>')
    .replace('                || (!write_back.is_empty() && name == "block_entity_tick")\n', ''))
rewrite(GEN / 'dispatch-tests/src/fixtures/shared.rs', lambda s: s.replace('BlockEntityTickReply, ', '').replace('::{DamageContainer};','::DamageContainer;'))
rewrite(GEN / 'dispatch-tests/src/fixtures/stubs/ModBlockEntityView.rs', lambda s: s[:s.index('    pub(super) fn __begin_tick_listener')] + '}\n')

for name in (
    'block_entity_tick_items.rs', 'block_entity_tick_reply.rs',
    'block_entity_tick_item_preconditions_tests.rs', 'block_entity_tick_items_bench_tests.rs',
    'block_entity_tick_items_tests.rs', 'block_entity_tick_reply_tests.rs',
    'block_entity_tick_reply_tests/fixture.rs', 'native_entity_refs/tick_item_read.rs',
    'native_entity_refs/tick_item_selector.rs', 'native_item_places/tick_item_read.rs',
    'native_item_entity_refs/tick_item_read.rs',
):
    (MODDING / name).unlink()
(GEN / 'dispatch-tests/src/fixtures/api/BlockEntityTickReply.rs').unlink()
Path('tools/test_block_entity_tick_items_mutants.py').unlink()
Path('tools/ci/ignored-tests/manual-sunrise-client-issue-4451.txt').unlink()
rewrite(Path('tools/ci/ignored-tests/daily-sunrise-client-issue-4451.txt'), lambda s: ''.join(
    line for line in s.splitlines(keepends=True) if 'game::block_entity_tick_item' not in line))
rewrite(Path('tools/ci/ignored-test-exceptions.json'), lambda s: json.dumps([
    row for row in json.loads(s) if 'game::block_entity_tick_items_bench_tests' not in row['test']], indent=2, ensure_ascii=False) + '\n')
