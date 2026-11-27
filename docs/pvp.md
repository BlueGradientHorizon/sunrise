# Client presentation

Optional client presentation settings, on **Options → Sunrise Settings... → Gameplay**. Every one is
off by default: a fresh install looks exactly like vanilla 26.3, and turning one off restores vanilla.

## Fair play

Nothing here tells the player more than vanilla does or plays for them, so the game stays fine on servers
that ban unfair clients. That rules out, on purpose: entities on the minimap, freelook, auto-clicking or any
other automatic action, seeing through walls, and hitboxes beyond vanilla's F3+B.
Each feature is built from what it visibly does, never from another client's or mod's code.

## Where it lives

| Part | Code |
|---|---|
| Settings, parsing, the page's rows | `crates/sunrise-client-render/src/pvp.rs` (stored in `sunrise-extras.txt`, see [extras.md](extras.md)) |
| Settings rows | `extras::rows()` includes `pvp::rows()` on Gameplay |
| Game glue | `crates/sunrise-client-gameplay/src/game/entities/pvp.rs` |

## Chat (each default off)

| Option | Key | What it does |
|---|---|---|
| Timestamps | `chatTimestamps` | The local time a message came in, before it, in grey (`sunrise.chat.timestamp`). |
| Compact chat | `chatCompact` | A message equal to the one before it (same text, kind and tag, neither signed) raises that one's count, "(x3)", instead of adding lines. |
| Copy on click | `chatCopy` | In the open chat, a click on a message where the click has no action of its own copies the message's text (without time or count). |
| Chat search | `chatSearch` | Ctrl+F in the open chat: the box takes a query and only the messages holding it (any case) show. Enter, Esc or Ctrl+F again stops. |

They change how `chat::Chat` lays its lines out (`ChatQol`, `set_search`): the 100 messages it keeps are the same,
so turning an option off shows them as before. Code: `chat.rs`, `crates/sunrise-client-gameplay/src/game/interface/chat.rs`.

## Combat feel (`combatFeel`, default Modern)

**Legacy (1.8.9)** draws fights as 1.8.9 did, from its own client (`ItemRenderer`, read for the numbers only):
the item held in first person sits, swings, eats, drinks and draws a bow along 1.8.9's poses, a sword is held up
to block while you hold use (with nothing in the off hand drawn), and the hand doesn't dip after each hit. The
attack indicator is hidden unless **Attack indicator in Legacy** (`combatIndicator`) keeps it. Items whose
resource pack moves them in hand, and the ones 1.8.9 didn't have (crossbow, trident, spear, brush...), keep
26.3's poses. 1.8.9 tinted no armour red on a hit, so the hurt flash is the same in both.
Legacy also leaves out what 1.9 added around a hit: the sweep arc and the damage hearts a server sends, and
with **1.8 hit sounds** (`combatSounds`) the `entity.player.attack.*` sounds (strong, weak, crit, sweep,
knockback, no damage); hurt sounds and crit particles stay. `pvp::Combat`.

It is only the look: the client sends a server exactly what Modern sends (same attacks, same timing, no
block-hit packets), so vanilla servers and their anticheats see nothing new. Code: `first_person.rs`
(`legacy_item_pose`), the hand in `game.rs`.
