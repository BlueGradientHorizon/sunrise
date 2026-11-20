# Randomness

A mod draws random numbers through one trait, `RandomSource`, with vanilla's draw names:
`next_int`, `next_int_bound`, `next_long`, `next_boolean`, `next_float`, `next_double` and
`next_gaussian`, plus two range helpers, `next_int_between_inclusive(min, max)` and
`next_int_range(origin, bound)`.

## Streams the game lends you

Wherever the game has a random stream a mod may draw from, you get a `&mut dyn RandomSource`:

| Where | Stream |
|---|---|
| `GenerationContext::random`, `PieceContext::random` | The structure start's or the piece's random |
| `GroundLevel::random` | The tree decorator's random |
| The tree-grow hook's random | The tree's own random |
| `GoalContext::random` | The mob's own random |
| `host.with_random(\|random\| ...)` on a restricted level host | A scoped borrow of the level's live random |
| `level.with_random(host, \|random\| ...)` on a `LevelRef` | The named level's random |

A draw moves the game's own stream, in vanilla's order, so your draws shift vanilla's later ones
exactly as a Java mod's would. You need no import to call these methods on a `dyn RandomSource`.

```rust,ignore
let roll = goal_context.random().next_int_bound(20);
// In a hook with a LevelRandomHost, HitHost, or LevelWriteHost:
let level_roll = host.with_random(|random| random.next_int_bound(20));
```

A restricted host returns `None` without calling the closure if it cannot lend the stream.
The closure cannot take the random with it or reseed it. On unwinding, the stream goes back to
where it was before the closure, including its cached Gaussian. On a transactional
`LevelWriteHost`, successful draws are also rolled back if the transaction aborts.

## Your own seeded stream

`LegacyRandomSource` is vanilla's `LegacyRandomSource` (the generator Java's `Random` is):
`LegacyRandomSource::new(seed)`, `set_seed(seed)`, and `RandomSource` for the draws (import the
trait to call them on the concrete type). Two sources with the same seed give the same values. It
is a plain value that crosses the boundary, and it is what `level_random` and `level_set_random`
carry.

## Never panics

Mod input never panics a draw. Where vanilla throws, `RandomSource` answers instead:

| Call | Answer |
|---|---|
| `next_int_bound(bound)` with `bound < 1` | 0, with no draw |
| `next_int_between_inclusive(min, max)` with `max < min` | `min`, with no draw |
| `next_int_range(origin, bound)` with `bound <= origin` | `origin`, with no draw |

Forked or positional sources (`forkPositional`) are not offered.
