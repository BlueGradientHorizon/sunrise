#!/usr/bin/env python3
"""Thread quickMoveStack's source slot to every moveItemStackTo call."""
from pathlib import Path

path = Path("crates/sunrise-client/src/menu.rs")
text = path.read_text()
start = text.index("    fn quick_move(&mut self, env: &Env, index: usize)")
end = text.index("/// `MerchantMenu` and `MerchantContainer`", start)
quick_move = text[start:end]
assert "self.move_to(env, Some(index)," not in quick_move
assert quick_move.count("self.move_to(env, &mut stack,") > 40
quick_move = quick_move.replace("self.move_to(env, &mut stack,", "self.move_to(env, Some(index), &mut stack,")
path.write_text(text[:start] + quick_move + text[end:])
