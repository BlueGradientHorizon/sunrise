# Implementation decisions

Behaviours the spec ([shader-packs-spec.md](shader-packs-spec.md), revision 1) leaves open, each with the decision,
its reason and the tests that hold it ([README](README.md), "Decisions on record"). An entry is never taken from another
implementation, its code, its issue tracker or its changelog. Decisions taken before this file existed (on the spec's
open questions, OQ-25, OQ-26 and others) are in the implementation plan, which was not published, so this file is not
the full list.

## D-1: which bolt `lightningBoltPosition` gives

- **Open point:** spec rev 1, 5.2, line 692 gives "player-space position of one rendered lightning bolt, w = 1; zero
  if none", without saying which bolt when several are drawn. It is not one of section 9's open questions.
- **Decision:** the newest bolt drawn this frame: the last of the frame's bolts, which are the ones within vanilla's
  draw distance (`LightningBolt.shouldRenderAtSqrDistance`; `LightningBoltRenderer.affectedByCulling` is false). Its
  position minus the frame's `cameraPosition` (player space, spec line 607), subtracted in double precision; zeros
  when none is drawn.
- **Reason (vanilla 26.3):** the client draws entities in the order the level added them
  (`ClientLevel.entitiesForRendering` iterates `EntityLookup`'s linked map; an id added again goes to the end), so the
  last drawn bolt is the newest one, the one whose flash the player sees last. A bolt never moves, so its render
  position is where it struck.
- **Tests:** `gfx::pack_uniforms::lightning_bolt_test::the_newest_drawn_bolt_reaches_the_pack_in_player_space`
  (`sunrise-client-render`) and the daily `game::lightning_bolt_uniform_tests::a_summoned_bolt_reaches_the_pack_in_player_space`
  (`sunrise-client`).
