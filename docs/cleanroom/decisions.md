# Implementation decisions

Behaviours the spec ([shader-packs-spec.md](shader-packs-spec.md), revision 1) leaves open, each with the decision,
its reason and the tests that hold it ([README](README.md), "Decisions on record"). An entry is never taken from another
implementation, its code, its issue tracker or its changelog. Decisions taken before this file existed (on the spec's
open questions, OQ-25, OQ-26 and others) are in the implementation plan, which was not published, so this file is not
the full list.

## D-2: which player booleans programs read, from which vanilla method, and their type

- **Open point:** spec rev 1, 3.4, line 552 lists fourteen player booleans (`is_alive` … `is_wet`) for expressions,
  "state of the player (the camera entity)", and adds "`[IR]` also exposes six of them as uniforms". The uniform
  table of 5.2 (line 664) lists only those six (`is_burning is_hurt is_invisible is_on_ground is_sneaking
  is_sprinting`). The bullet at line 558 ("the player booleans as normal uniforms too") can be read as all fourteen.
  The spec names no vanilla method for any of them, and no type beyond "bool". None of this is one of section 9's
  open questions.
- **Decision 1, expression-only:** custom uniforms (`uniform.<type>.<name> = <expression>`) read all fourteen. A
  program that declares one of the eight others (`is_alive is_child is_glowing is_in_lava is_in_water is_ridden
  is_riding is_wet`) gets no built-in value: it reads 0, unless the pack defines a custom uniform of that name, which
  binds instead. Compute programs and quad programs follow the same rule. The basis is line 552's "six of them" and
  the six-name table at line 664, which agree; line 558 is the looser of the three and is read through them. The 3.4
  heading ("Parameters only for expressions (Tier A) and as uniforms (Tier E)") is no basis: `biome`, `temperature`
  and the rest sit under it and are program uniforms.
- **Decision 2, the vanilla method behind each name** (Minecraft 26.3, the window's player as a client answers;
  `LocalPlayer` and `AbstractClientPlayer` override none of them):
  - `is_alive`: `LivingEntity.isAlive` (`!isRemoved() && getHealth() > 0`; the window's player is never removed
    while it is the camera);
  - `is_child`: `LivingEntity.isBaby`, which `Player` keeps, so always 0;
  - `is_glowing`: `Entity.isCurrentlyGlowing` on a client, which skips the effect check and reads shared flag 6: the
    flag a server sends, or in single player the Glowing effect the flag is set from (a local `Glowing` tag is not
    modelled);
  - `is_in_lava`: `Entity.isInLava` (`!firstTick && isInFluid(LAVA)`);
  - `is_in_water`: `Entity.isInWater` (`wasTouchingWater`), not `isUnderWater`;
  - `is_ridden`: `Entity.isVehicle` (`!passengers.isEmpty()`), so a dying passenger counts until it is removed;
  - `is_riding`: `Entity.isPassenger` (`getVehicle() != null`);
  - `is_wet`: `Entity.isInWaterOrRain` (`isInWater() || isInRain()`, rain at `blockPosition` or at the box's top);
  - the six of the table keep the methods they already had.
- **Decision 3, bool only:** the eight are `bool` built-ins, like the six, so `uniform.float.x = is_wet` reads 0 as
  `uniform.float.x = is_sneaking` does. Only the six were probed; the eight are bool by inference from the spec's
  "bool" and from the six, not by measurement.
- **Not covered:** the spec's "the camera entity". While spectating another entity all fourteen read the window's
  player, as the six did before this entry.
- **Tests:** `gfx::pack_uniforms::initial_history_tests::custom_uniforms_read_every_player_boolean`,
  `gfx::pack_uniforms::initial_history_tests::programs_read_only_six_player_booleans` and
  `gfx::pack_post::player_boolean_tests::compute_and_quad_programs_read_only_the_listed_player_booleans`
  (`sunrise-client-render`); the daily `game::shader_packs::player_status_tests::pack_status_follows_the_player`,
  `game::shader_packs::player_status_tests::pack_status_leaves_queued_effects_alone` and
  `game::shader_packs::player_status_tests::pack_status_on_a_server_follows_what_it_sent` (`sunrise-client`).
