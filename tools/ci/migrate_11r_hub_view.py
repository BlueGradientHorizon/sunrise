#!/usr/bin/env python3
"""Migrate the Hub slot-view fixtures from live guests to staged click state."""

from pathlib import Path
import re

path = Path('crates/sunrise-client/src/lan/menu.rs')
source = path.read_text()
source = re.sub(
    r'(?m)^( +)guest: &mut (g|guest),$',
    r'\1guest: &mut ClickPlayer::read(&\2),\n\1open: \2.menu.as_ref(),',
    source,
)
path.write_text(source)

# A guest no longer needs a mutable borrow just to lend the click its opening.
scope = source.index('#[cfg(test)]')
fixtures = source[scope:]

def drop_unused_mut(match):
    name, block = match.groups()
    body = block.split(f'ClickPlayer::read(&{name})', 1)[0]
    if not re.search(rf'\b{name}\.[a-z_]+(?:[.\[]|\s*=|\.as_mut)', body):
        return f'let {name} = guest();{block}'
    return match[0]

fixtures = re.sub(r'let mut (g|guest) = guest\(\);(.*?ClickPlayer::read\(&\1\))',
                  drop_unused_mut, fixtures, flags=re.S)
path.write_text(source[:scope] + fixtures)
