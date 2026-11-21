# Implementation decisions

Behaviours the spec ([shader-packs-spec.md](shader-packs-spec.md), revision 1) leaves open, each with the decision,
its reason and the tests that hold it ([README](README.md), "Decisions on record"). An entry is never taken from another
implementation, its code, its issue tracker or its changelog. Decisions taken before this file existed (on the spec's
open questions, OQ-25, OQ-26 and others) are in the implementation plan, which was not published, so this file is not
the full list.

## D-1: which bolt `lightningBoltPosition` gives

- **Open point:** spec rev 1, 5.2, line 692 gives "player-space position of one rendered lightning bolt, w = 1; zero
  if none", without saying which bolt when several are drawn. It is not one of section 9's open questions.
- **Decision:** the last bolt in Sunrise's draw list: the frame's bolts (`bolt_instances`), which are the ones within
  vanilla's draw distance (`LightningBolt.shouldRenderAtSqrDistance`; `LightningBoltRenderer.affectedByCulling` is
  false), in the order the game keeps them (a bolt added later goes last). Its offset from the frame's `cameraPosition`
  (player space, spec line 607), subtracted in double precision by the same function that places the bolt's geometry
  (`gfx::lightning::offset`); zeros when the list is empty.
- **Reason:** the spec asks for one rendered bolt, and the draw list is what Sunrise renders, so the uniform and the
  drawn geometry agree. The order bolts are submitted in doesn't decide whose pixels end up on screen last, so this is
  not a claim about which flash the player sees last; the uniform only follows the existing list. Which bolts are
  visible and how they sort is separate rendering work. A bolt never moves, so its render position is where it
  struck.
- **Tests:** `gfx::pack_uniforms::lightning_bolt_test::the_last_bolt_in_the_draw_list_reaches_the_pack_in_player_space`
  (`sunrise-client-render`) and the daily `game::lightning_bolt_uniform_tests::a_summoned_bolt_reaches_the_pack_in_player_space`
  (`sunrise-client`).
