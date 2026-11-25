#[allow(unused_imports)]
use super::*;
#[allow(unused_imports)]
use mod_api::ModBlockEntity;
impl GameLevel<'_> {
    /// Initial saved data for a declared block entity (`EntityBlock.newBlockEntity`),
    /// fired inside the write that creates it (`LevelChunk.setBlockState`), on
    /// a server and on a client of one. The declaration owns the type id; no
    /// answer starts with an empty compound. The level is read-only, as
    /// `newBlockEntity` gets no level.
    #[inline(always)]
    pub(in crate::game) fn native_block_entity(
        &self,
        block: &str,
        state: BlockStateId,
        entity: &mut Option<Compound>,
    ) {
        self.fire_block_entity(block, state, entity);
    }
    /// A hopper at `pos` is about to pick up `count` of `item`. Set `cancel` to leave the dropped stack in the world. Server-side.
    #[inline(always)]
    pub(in crate::game) fn native_hopper_pickup_item(
        &self,
        dimension: LevelDimension,
        pos: BlockPos,
        item: &str,
        count: u32,
        cancel: &mut bool,
    ) {
        self.fire_hopper_pickup_item(dimension, pos, item, count, cancel);
    }
    /// An authoritative server committed a changed state through `Level.setBlock`.
    /// The world contains `new`; block side effects and neighbour updates follow.
    #[inline(always)]
    pub(in crate::game) fn native_block_changed(
        &self,
        dimension: LevelDimension,
        pos: BlockPos,
        old: BlockStateId,
        new: BlockStateId,
        flags: BlockUpdateFlags,
    ) {
        self.fire_block_changed(dimension, pos, old, new, flags);
    }
    /// A flowing fluid at `pos` is about to change level. `fluid` is `water`
    /// or `lava`; the levels are 0 through 8. Set `cancel` to keep the old
    /// level. Server-side.
    #[inline(always)]
    pub(in crate::game) fn native_fluid_level_change(
        &self,
        dimension: LevelDimension,
        pos: BlockPos,
        fluid: &str,
        from: i32,
        to: i32,
        cancel: &mut bool,
    ) {
        self.fire_fluid_level_change(dimension, pos, fluid, from, to, cancel);
    }

    /// A non-persistent leaf block at distance 7 is about to decay at `pos`.
    /// Set `cancel` to keep it. Server-side.
    #[inline(always)]
    pub(in crate::game) fn native_leaves_decay(
        &self,
        dimension: LevelDimension,
        pos: BlockPos,
        cancel: &mut bool,
    ) {
        self.fire_leaves_decay(dimension, pos, cancel);
    }

    /// Farmland at `pos` is about to change moisture from `from` to `to`.
    /// Set `cancel` to keep its old moisture. Server-side.
    #[inline(always)]
    pub(in crate::game) fn native_moisture_change(
        &self,
        dimension: LevelDimension,
        pos: BlockPos,
        from: i32,
        to: i32,
        cancel: &mut bool,
    ) {
        self.fire_moisture_change(dimension, pos, from, to, cancel);
    }

    /// A sponge at `pos` absorbed water and became wet. Server-side.
    #[inline(always)]
    pub(in crate::game) fn native_sponge_absorb(&self, dimension: LevelDimension, pos: BlockPos) {
        self.fire_sponge_absorb(dimension, pos);
    }

    /// A block at `pos` is about to fade from `from` to `to` because of
    /// melting or coral death. Set `cancel` to keep it. Server-side.
    #[inline(always)]
    pub(in crate::game) fn native_block_fade(
        &self,
        dimension: LevelDimension,
        pos: BlockPos,
        from: BlockStateId,
        to: BlockStateId,
        cancel: &mut bool,
    ) {
        self.fire_block_fade(dimension, pos, from, to, cancel);
    }

    /// Precipitation is about to form or add to a block at `pos`. `from` and
    /// `to` are block state IDs. Set `cancel` to keep the old state. Server-side.
    #[inline(always)]
    pub(in crate::game) fn native_block_form(
        &self,
        dimension: LevelDimension,
        pos: BlockPos,
        from: BlockStateId,
        to: BlockStateId,
        cancel: &mut bool,
    ) {
        self.fire_block_form(dimension, pos, from, to, cancel);
    }

    /// The Ender Dragon fight is about to place its first dragon egg at `pos`.
    /// Set `cancel` to leave the block unchanged. Server-side.
    #[inline(always)]
    pub(in crate::game) fn native_dragon_egg_form(
        &self,
        dimension: LevelDimension,
        pos: BlockPos,
        cancel: &mut bool,
    ) {
        self.fire_dragon_egg_form(dimension, pos, cancel);
    }

    /// An authoritative server is about to deliver `neighborChanged`.
    /// State and block IDs are valid in this level's registries for this call.
    /// The host is the level writer vanilla's `neighborChanged` gets: a part may read
    /// its inputs (`signal`, `has_neighbor_signal`), schedule its tick, as a diode does
    /// (`DiodeBlock.checkTickOnNeighbor`), and set blocks, as `RedstoneLampBlock` lights
    /// itself at once (`level.setBlock(pos, state.cycle(LIT), 2)`). A write's own
    /// neighbour updates run next, before the rest of the update that delivered this
    /// call (`CollectingNeighborUpdater`), as `Level.setBlock` from inside
    /// `neighborChanged` runs them. The block's own handling then runs with the state
    /// it was called with.
    #[inline(always)]
    pub(in crate::game) fn native_neighbor_changed(
        &mut self,
        ticks: &mut Ticks,
        dimension: LevelDimension,
        pos: BlockPos,
        state: BlockStateId,
        changed: mod_api::BlockKey,
        moved: bool,
    ) {
        self.fire_neighbor_changed(ticks, dimension, pos, state, changed, moved);
    }

    /// On an authoritative server, a fluid flows or fire spreads from `from`
    /// into `to` (`kind` is `fluid` or `fire`): `FlowingFluid.spreadTo`, and
    /// `FireBlock.tick` setting fire to `to` or burning it (`checkBurnOut`).
    /// Set `canceled` to leave the block at `to` as it is. Fluids ask on
    /// every flow: answer fast.
    #[inline(always)]
    pub(in crate::game) fn native_block_spread(
        &self,
        dimension: LevelDimension,
        kind: &str,
        from: BlockPos,
        to: BlockPos,
        canceled: &mut bool,
    ) {
        self.fire_block_spread(dimension, kind, from, to, canceled);
    }

    /// On an authoritative server, fire is about to be placed at `pos` by
    /// flint and steel, a fire charge, lava, lightning or fire spread. Set
    /// `player` is the acting server player when there is one. Set `cancel` to
    /// leave the position unchanged.
    #[inline(always)]
    pub(in crate::game) fn native_block_ignite(
        &self,
        dimension: LevelDimension,
        player: Option<u128>,
        pos: BlockPos,
        cause: &str,
        cancel: &mut bool,
    ) {
        self.fire_block_ignite(dimension, player, pos, cause, cancel);
    }

    /// On an authoritative server, fire is about to burn the block at `pos`
    /// away. Set `cancel` to keep that block.
    #[inline(always)]
    pub(in crate::game) fn native_block_burn(
        &self,
        dimension: LevelDimension,
        pos: BlockPos,
        cancel: &mut bool,
    ) {
        self.fire_block_burn(dimension, pos, cancel);
    }

    /// On an authoritative server, a bell at `pos` is about to ring. Set
    /// `cancel` to stop the ring and its raider reveal.
    #[inline(always)]
    pub(in crate::game) fn native_bell_ring(
        &self,
        dimension: LevelDimension,
        player: Option<u128>,
        pos: BlockPos,
        cancel: &mut bool,
    ) {
        self.fire_bell_ring(dimension, player, pos, cancel);
    }

    /// On an authoritative server, a cauldron at `pos` is about to change
    /// fill level from `from_level` to `to_level` for `reason`. Set `cancel`
    /// to leave it unchanged.
    #[inline(always)]
    pub(in crate::game) fn native_cauldron_level_changed(
        &self,
        call: mod_api::__private::CauldronLevelChangedCall<'_>,
    ) {
        self.fire_cauldron_level_changed(call);
    }

    /// On an authoritative server, the piston at `piston` is about to push
    /// the blocks at `push` one step `direction` (`[0, 1, 0]` is up; pulling
    /// when it retracts, `extending` false) and destroy those at `destroy`:
    /// NeoForge's `PistonEvent.Pre`, in `PistonBaseBlock.moveBlocks`, once its
    /// structure resolved. Set `canceled` and nothing moves, as when the
    /// structure can't; a sticky piston then retracts without its block, and
    /// clients are sent the blocks rather than the retraction to replay.
    #[inline(always)]
    pub(in crate::game) fn native_piston_pre(&self, call: mod_api::__private::PistonPreCall<'_>) {
        self.fire_piston_pre(call);
    }
    /// On an authoritative server, the piston at `piston` pushed the blocks
    /// at `push` one step `direction` and destroyed those at `destroy`:
    /// NeoForge's `PistonEvent.Post`, at the end of
    /// `PistonBaseBlock.moveBlocks`, after the neighbour updates. It only
    /// hears; `native_piston_pre` is the veto.
    #[inline(always)]
    pub(in crate::game) fn native_piston_post(
        &self,
        dimension: LevelDimension,
        piston: BlockPos,
        direction: [i32; 3],
        extending: bool,
        push: &[BlockPos],
        destroy: &[BlockPos],
    ) {
        self.fire_piston_post(dimension, piston, direction, extending, push, destroy);
    }
    /// The redstone signal the block `state` at `pos` gives on its `face`, the
    /// side that touches the block reading it: `BlockState.getDirectSignal`
    /// when `direct` (what passes through a block it touches, as a lever's
    /// does), else `getSignal`. `signal` holds vanilla's answer: change it when
    /// the block is yours (0 to 15; anything else is clamped). Asked very often,
    /// from inside the level's update, so answer from your own state: check the
    /// block first, and don't read or change the world here. When the answer
    /// changes, and once the block is placed, call
    /// `level.update_neighbors(host, pos, state, NeighbourUpdate::Neighbours)`. Authoritative
    /// servers only.
    #[inline(always)]
    pub(in crate::game) fn native_block_signal(
        &self,
        dimension: LevelDimension,
        pos: BlockPos,
        state: BlockStateId,
        face: Direction,
        direct: bool,
        signal: &mut i32,
    ) {
        self.fire_block_signal(dimension, pos, state, face, direct, signal);
    }

    /// `BlockState.isSignalSource`: set `source` for a block of yours that
    /// gives a signal, so redstone wire beside it connects to it. Authoritative
    /// servers only.
    #[inline(always)]
    pub(in crate::game) fn native_is_signal_source(&self, state: BlockStateId, source: &mut bool) {
        self.fire_is_signal_source(state, source);
    }

    /// `BlockState.hasAnalogOutputSignal`: set `output` for a block of yours
    /// that comparators read (how full or far along it is, as a composter's
    /// level), so they ask `native_analog_output`. Asked from inside the
    /// level's update: answer from `state` alone. Authoritative servers only.
    #[inline(always)]
    pub(in crate::game) fn native_has_analog_output(&self, state: BlockStateId, output: &mut bool) {
        self.fire_has_analog_output(state, output);
    }

    /// `BlockState.getAnalogOutputSignal`: the signal a comparator reads from
    /// the block `state` at `pos`, `face` the side that touches the comparator
    /// (or the conductor in front of it). `output` holds vanilla's answer:
    /// change it when the block is yours (0 to 15; anything else is clamped).
    /// Asked only for blocks `native_has_analog_output` answered for, from
    /// inside the level's update: don't change the world here. When the
    /// answer changes without the block's state changing, call
    /// `level.update_neighbors(host, pos, state, NeighbourUpdate::OutputSignal)`; a change of
    /// state through
    /// `native_set_block` or `native_set_block_property` (neighbour updates
    /// on) tells the comparators itself, as vanilla's `Level.setBlock` does.
    /// Authoritative servers only.
    #[inline(always)]
    pub(in crate::game) fn native_analog_output(
        &mut self,
        dimension: LevelDimension,
        pos: BlockPos,
        state: BlockStateId,
        face: Direction,
        output: &mut i32,
    ) {
        self.fire_analog_output(dimension, pos, state, face, output);
    }

    /// An authoritative server's `ServerLevel.tickChunk` is about to
    /// random-tick the block `state` at `pos` (`BlockState.randomTick`):
    /// only blocks that tick randomly, a mod's as its vanilla template block
    /// does. Bone meal's random ticks (`StemBlock.performBonemeal`) don't
    /// fire it. Change the block with `native_set_block_property` and set
    /// `handled` to skip the block's own random tick (a mod crop growing its
    /// own way); a block changed without `handled` takes its own random tick
    /// as it is now, if it still ticks randomly. Lava's fluid random tick
    /// runs either way.
    #[inline(always)]
    pub(in crate::game) fn native_random_tick(
        &mut self,
        ticks: &mut Ticks,
        dimension: LevelDimension,
        pos: BlockPos,
        state: BlockStateId,
        handled: &mut bool,
    ) {
        self.fire_random_tick(ticks, dimension, pos, state, handled);
    }

    /// An authoritative server is about to compute the state the block
    /// `state` at `pos` takes now that its neighbour in `direction` is
    /// `neighbour` (`BlockBehaviour.updateShape`). `state` may not be in the
    /// world yet (a piston landing it): decide from `state`, not from the
    /// block at `pos`. Answer only for your own blocks: put the new state in
    /// `new` (`state` keeps the block as it is; air breaks it, with its drops,
    /// on a neighbour update) and the block's own `updateShape` doesn't run,
    /// nor the ticks it would schedule. The level is read-only, as vanilla's
    /// `LevelReader`; ticks go through `native_schedule_block_tick` and
    /// `native_schedule_fluid_tick` with `ticks` (`ScheduledTickAccess`). It
    /// runs often, at least six times per `setBlock` that updates shapes
    /// (more for redstone wire's diagonals and knock-on updates): keep it
    /// short.
    #[inline(always)]
    pub(in crate::game) fn native_update_shape(
        &self,
        ticks: &mut Ticks,
        call: super::fire::UpdateShapeCall<'_>,
    ) {
        self.fire_update_shape(ticks, call);
    }

    /// An authoritative server's `ServerLevel.tickBlock` is about to run a
    /// due scheduled tick of the block `state` at `pos` (`BlockState.tick`):
    /// the block is still the one the tick was scheduled for. Change the
    /// block with `native_set_block_property` and set `handled` to skip the
    /// block's own tick; a block changed without `handled` takes its own
    /// tick as it is now, if it is still the tick's block.
    #[inline(always)]
    pub(in crate::game) fn native_tick_block(
        &mut self,
        ticks: &mut Ticks,
        dimension: LevelDimension,
        pos: BlockPos,
        state: BlockStateId,
        handled: &mut bool,
    ) {
        self.fire_tick_block(ticks, dimension, pos, state, handled);
    }

    /// A block nothing in Sunrise knows a `BonemealableBlock` for is about to
    /// take bone meal: vanilla's `BonemealableBlock.isValidBonemealTarget`,
    /// `state` the block's own state at `pos`. Answer `valid` for your own
    /// blocks: the meal is used up on one as vanilla uses it up (the growth
    /// particles and the sound follow, whether or not the roll below grows
    /// it), from a player's `BoneMealItem.useOn`, from a dispenser and, on a
    /// crop (a villager bonemeals only a `CropBlock`), from a villager. Asked
    /// on both sides, as vanilla asks it on both, so a client predicts the
    /// use with your answer: say no where you can only grow on a server
    /// (match `LevelSide`). `Some(false)` refuses the meal, even on a block
    /// whose template takes it (vanilla's `growCrop` then keeps the meal);
    /// left `None`, the block keeps the `BonemealableBlock` its template gives
    /// it, or takes no bone meal.
    #[inline(always)]
    pub(in crate::game) fn native_is_valid_bonemeal_target(
        &self,
        dimension: LevelDimension,
        pos: BlockPos,
        state: BlockStateId,
        valid: &mut Option<bool>,
    ) {
        self.fire_is_valid_bonemeal_target(dimension, pos, state, valid);
    }

    /// `BonemealableBlock.isBonemealSuccess` for a block that answered
    /// `native_is_valid_bonemeal_target`, after it did: set `success` to grow
    /// it this time. Vanilla draws the level's random here
    /// (`isBonemealSuccess(Level, RandomSource, BlockPos, BlockState)`), so
    /// listeners draw from the live stream with
    /// `host.with_random(|rng| rng.next_int_bound(bound))`.
    /// Authoritative servers only.
    #[inline(always)]
    pub(in crate::game) fn native_is_bonemeal_success(
        &mut self,
        dimension: LevelDimension,
        pos: BlockPos,
        state: BlockStateId,
        success: &mut bool,
    ) {
        self.fire_is_bonemeal_success(dimension, pos, state, success);
    }

    /// `BonemealableBlock.performBonemeal` for a block that answered
    /// `native_is_valid_bonemeal_target` and whose `native_is_bonemeal_success`
    /// said yes: grow it here, `state` its state at `pos`. The meal is used up
    /// either way. Change the block with the level's
    /// `native_set_block_property`, and schedule with `native_schedule_block_tick`
    /// (`ticks` is the hook's). Authoritative servers only.
    #[inline(always)]
    pub(in crate::game) fn native_perform_bonemeal(
        &mut self,
        ticks: &mut Ticks,
        dimension: LevelDimension,
        pos: BlockPos,
        state: BlockStateId,
    ) {
        self.fire_perform_bonemeal(ticks, dimension, pos, state);
    }

    /// On both sides, `CampfireBlock.isSmokeSource` for the
    /// block `state` under a campfire, as the campfire is placed or the block
    /// under it changes: `Some(true)` for your own blocks makes the campfire a
    /// signal fire whose smoke rises higher, as over a hay bale. The level is
    /// read-only.
    #[inline(always)]
    pub(in crate::game) fn native_smoke_source(
        &self,
        dimension: LevelDimension,
        state: BlockStateId,
        smoke: &mut Option<bool>,
    ) {
        self.fire_smoke_source(dimension, state, smoke);
    }
}

/// The hook points. Each does nothing in Sunrise.
impl Game<'_> {
    /// Before an authoritative server level's tick pass. `dimension` is its ID.
    pub(in crate::game) fn native_level_tick_pre(&mut self, level: LevelRef) {
        self.fire_level_tick_pre(level);
    }
    /// After a successful authoritative server level tick pass.
    pub(in crate::game) fn native_level_tick_post(&mut self, level: LevelRef) {
        self.fire_level_tick_post(level);
    }
    /// `Player.interactOn`: both logical sides answer before entity interaction.
    /// Only the client's own consuming answer stops its hand loop.
    pub(in crate::game) fn native_entity_interact(
        &mut self,
        hand: Hand,
        entity: i32,
        kind: &str,
        stack: Option<&ItemStack>,
        result: &mut Option<mod_api::InteractionResult>,
    ) {
        self.fire_entity_interact(hand, entity, kind, stack, result);
    }
    /// NeoForge's non-cancellable PlayerTickEvent.Pre on either logical side.
    pub(in crate::game) fn native_player_tick_pre(
        &mut self,
        call: mod_api::__private::PlayerTickPreCall,
    ) {
        self.fire_player_tick_pre(call);
    }
    /// NeoForge's non-cancellable PlayerTickEvent.Post on either logical side.
    pub(in crate::game) fn native_player_tick_post(
        &mut self,
        call: mod_api::__private::PlayerTickPostCall,
    ) {
        self.fire_player_tick_post(call);
    }
    /// Whether block `block`'s block entity ticks (`EntityBlock.getTicker`):
    /// set `ticks` when it is yours. Asked for each of the block's states
    /// when a world opens.
    /// Set `ticks` to true to make it tick, and never clear a value an earlier mod or mixin set.
    pub(in crate::game) fn native_block_entity_ticks(&self, block: &str, ticks: &mut bool) {
        self.fire_block_entity_ticks(block, ticks);
    }
    /// Block `block`'s registered item slots (`WorldlyContainer`), read from
    /// its per-block declaration when the ticking table is built.
    /// Registration already checks the saved slot count. The table builder leaves
    /// out slot references past that count.
    /// Its menu must be one of the supported menus:
    /// `containers::MOD_MENUS` or a content mod's menu type
    /// (`data/<id>/sunrise/menu/`) with as many
    /// slots, opens when the block is used (its template must open a menu, as
    /// a chest or a furnace does); another is left out. A mod menu type shows
    /// on Sunrise clients through the mod's registered screen factory.
    pub(in crate::game) fn native_block_entity_container(
        &self,
        block: &str,
        container: &mut Option<ModContainer>,
    ) {
        *container = mod_api::__private::parts_installation::block_container(block).cloned();
    }
    /// Block `block`'s registered fluid tanks: each tank's capacity in
    /// millibuckets, the tanks each face reaches, and the tanks a fill may
    /// reach and a drain may take. Read from its per-block declaration when the
    /// ticking table is built; at most 16 tanks, capacities clamped to
    /// 1..=1 000 000, and tank numbers past the count are left out. The tanks'
    /// contents are kept in the entity's saved compound, under `fluids`, and
    /// the fluid helpers below read and write them.
    pub(in crate::game) fn native_block_entity_fluids(
        &self,
        block: &str,
        fluids: &mut Option<ModFluidTanks>,
    ) {
        *fluids = mod_api::__private::parts_installation::block_fluids(block);
    }
    /// Block `block`'s registered energy buffer (`Capabilities.Energy.BLOCK`):
    /// its capacity, the most one insert and one extract moves, and the faces
    /// a fill may come through and a drain may leave by.
    /// Read from its per-block declaration when the ticking
    /// table is built; a capacity under 1 leaves the buffer out, and the
    /// limits are clamped to 0..=capacity. The amount is kept in the entity's
    /// saved compound, as the int `energy`, and the energy helpers below read
    /// and write it.
    pub(in crate::game) fn native_block_entity_energy(
        &self,
        block: &str,
        energy: &mut Option<ModEnergyBuffer>,
    ) {
        *energy = mod_api::__private::parts_installation::block_energy(block);
    }
    /// Whether the owner of this menu accepts button `id`. The active menu's owner
    /// decides its own IDs; the game does not impose a shared numeric range.
    pub(in crate::game) fn native_menu_button_valid(
        &self,
        menu: &str,
        block: Option<&str>,
        id: i32,
        accepted: &mut bool,
    ) {
        self.fire_menu_button_valid(menu, block, id, accepted);
    }

    /// A server broadcast line is about to be logged and sent. Change `line`
    /// or set `cancel` to drop it.
    pub(in crate::game) fn native_broadcast_message(&self, line: &mut Text, cancel: &mut bool) {
        self.fire_broadcast_message(line, cancel);
    }
    /// A hopper is about to move one item between containers, a composter's
    /// bone meal included. Set `cancel` to leave it in its source slot. A chest
    /// boat or minecart is reported at the block it is in.
    pub(in crate::game) fn native_inventory_move_item(
        &self,
        source: [i32; 3],
        destination: [i32; 3],
        item: &str,
        cancel: &mut bool,
    ) {
        self.fire_inventory_move_item(source, destination, item, cancel);
    }
    /// `ItemStack.useOn`: both logical sides answer after the block passed.
    /// Client reads and local effects cannot perform authoritative writes.
    pub(in crate::game) fn native_use_on(&mut self, call: mod_api::__private::UseOnCall<'_>) {
        self.fire_use_on(call);
    }
    /// A player uses `call.stack` with this click's `call.hand`: in the air, or
    /// on a block once `native_use_on` let it through, where `Item.use`
    /// starts. Both sides return the full interaction result; Pass continues.
    /// Consume handles without swinging. Cooldown items are skipped.
    pub(in crate::game) fn native_use_item(&mut self, call: mod_api::__private::UseItemCall<'_>) {
        self.fire_use_item(call);
    }
    /// `Item.getUseDuration` of `stack`, in ticks: `duration` arrives with
    /// vanilla's (a consumable's time, 72000 for a shield, a spear, a bow,
    /// 0 for an item with no use over time). A mod's own item (outside
    /// `minecraft:`) with no `consumable`, `blocks_attacks` or kinetic
    /// weapon that answers above 0 is used over time on a right click, as an
    /// `Item.use` that calls `startUsingItem`, until it is let go or the time
    /// runs out (`native_living_entity_use_item_finish`). A consumable's time
    /// changes too, potions and the other vanilla items keep their own.
    /// Asked when a use starts, each tick a mod's item is in use, and each
    /// frame the hand draws one in use, on both sides. It has no user, as
    /// it is asked so often: answer from the stack alone.
    pub(in crate::game) fn native_use_duration(&self, stack: &ItemStack, duration: &mut i32) {
        self.fire_use_duration(stack, duration);
    }
    /// `Item.getUseAnimation` of `stack` in use, with the item's vanilla
    /// answer as the starting animation. Asked on the client each first-person
    /// frame and each local or remote player model tick. A spear override is
    /// refused if the item has no kinetic data.
    pub(in crate::game) fn native_use_animation(
        &self,
        stack: &ItemStack,
        animation: &mut UseAnimation,
    ) {
        self.fire_use_animation(stack, animation);
    }
    /// `Player.canEat` for a food `stack`, or None for cake and candle cake
    /// regardless of the held item. `player` is the eater on either side.
    /// Vanilla uses always-eat false for cake or the food component for a stack;
    /// `vanilla_can_eat` is that answer (creative, always-eat, or hungry).
    /// Answer `Some(false)` to refuse the food,
    /// `Some(true)` to let a full player eat; `can_eat` left `None` keeps
    /// `vanilla_can_eat`. Both sides (the client starts eating as the
    /// server does), so answer from the stack and the player alone.
    pub(in crate::game) fn native_can_eat(
        &self,
        player: PlayerRef,
        stack: Option<&ItemStack>,
        vanilla_can_eat: bool,
        can_eat: &mut Option<bool>,
    ) {
        self.fire_can_eat(player, stack, vanilla_can_eat, can_eat);
    }
    /// NeoForge's `LivingEntityUseItemEvent.Start`: `entity` starts using
    /// `stack` in `hand` (`LivingEntity.startUsingItem`), for `duration`
    /// ticks (`native_use_duration`'s answer). `entity` is a mob's
    /// `Entity.getId`, None for a player (`native_acting_player` says who);
    /// a mob fires it only for `LivingRef::start_using_item`. Change `duration` (consumables, potions,
    /// spyglasses and mods' items count to it; below 0 refuses the use, and
    /// 0 runs it out on its first tick, as `native_living_entity_use_item_tick`
    /// running it out, unless the item's own is 0 already), or set
    /// `canceled` and the use doesn't start. Both sides, as
    /// vanilla's: on the server for every player of the world, and on a
    /// server's client for the uses it predicts. Answer the same on both,
    /// and change the world only on the server (`native_window` is false
    /// there), not on a server's client.
    pub(in crate::game) fn native_living_entity_use_item_start(
        &mut self,
        stack: &ItemStack,
        entity: Option<i32>,
        hand: Hand,
        duration: &mut i32,
        canceled: &mut bool,
    ) {
        self.fire_living_entity_use_item_start(stack, entity, hand, duration, canceled);
    }
    /// NeoForge's `LivingEntityUseItemEvent.Finish`: `entity`'s use of
    /// `stack` in `hand` ran its time (`LivingEntity.completeUsingItem`),
    /// `duration` the ticks left, always 0. `entity` is as in
    /// `native_living_entity_use_item_start`. `result` arrives with what
    /// `finishUsingItem` left in the hand (the bowl of a stew, none for the
    /// last apple; a mod's own item is kept) and may be replaced. Fired for
    /// consumables, potions, spyglasses and mods' items. Both sides (a server's client sees its prediction);
    /// on the server for every player of the world (the helpers read the acting one).
    pub(in crate::game) fn native_living_entity_use_item_finish(
        &mut self,
        stack: &ItemStack,
        entity: Option<i32>,
        hand: Hand,
        duration: i32,
        result: &mut Option<ItemStack>,
    ) {
        self.fire_living_entity_use_item_finish(stack, entity, hand, duration, result);
    }
    /// Each living tick that `entity` may still use `stack` in `hand`.
    /// `entity` is as in `native_living_entity_use_item_start`; `duration`
    /// replaces the remaining-ticks counter, not the original use duration.
    /// The order is the may-still-use check, this event, the per-tick effects
    /// only while the counter is above 0, then the decrement and server finish.
    ///
    /// On the server, canceling (which leaves -1), or setting 0 or below,
    /// finishes the use after the decrement: food is eaten, potions applied,
    /// `native_living_entity_use_item_finish` fires, and the use is cleared.
    /// Setting 1 runs the effects and finishes in this same tick. This path
    /// never calls release-using: a bow fires nothing. A use-on-release item
    /// (a crossbow) does not finish here; its counter goes negative until release.
    /// On the client cancellation sets the counter to -1 before decrement and
    /// skips effects; the negative countdown continues until server completion or release.
    ///
    /// In contrast, canceling Start prevents the use beginning (duration 0
    /// does begin it); canceling Stop skips only release-using. Finish is not
    /// cancellable, but its result stack may be replaced. Both sides: change
    /// the world only on the server, not on a server's client. Runs every tick
    /// of every use; check the item first.
    pub(in crate::game) fn native_living_entity_use_item_tick(
        &mut self,
        stack: &ItemStack,
        entity: Option<i32>,
        hand: Hand,
        duration: &mut i32,
        canceled: &mut bool,
    ) {
        self.fire_living_entity_use_item_tick(stack, entity, hand, duration, canceled);
    }
    /// `Item.onUseTick` of `stack`, used by `entity` with `remaining` ticks
    /// left, always above 0: what a mod's own item does each tick it is used
    /// (cook, charge, drain). It comes where `ItemStack.onUseTick` calls the
    /// item, once `native_living_entity_use_item_tick` let the use go on:
    /// after a consumable's crumbs, for consumables, shields and mods' items
    /// (vanilla's other items do their own, and a kinetic weapon has none on
    /// the server). Both sides, as `native_living_entity_use_item_tick`:
    /// change the world only where this `Game` runs the world, not on a
    /// server's client. Runs every tick of every use: check the item first.
    pub(in crate::game) fn native_on_use_tick(
        &mut self,
        stack: &ItemStack,
        entity: Option<i32>,
        remaining: i32,
    ) {
        self.fire_on_use_tick(stack, entity, remaining);
    }
    /// NeoForge's `LivingEntityUseItemEvent.Stop`: `entity` lets go of
    /// `stack` in `hand` before its use ran its time
    /// (`LivingEntity.releaseUsingItem`: the use button let go, a screen
    /// opened), `duration` the ticks left. Set `canceled` to skip the item's
    /// release (a bow doesn't shoot, a spyglass makes no sound,
    /// `native_release_using` isn't called); the use stops anyway. Not when
    /// the hand changed item, which stops the use without a release. Both
    /// sides, as `native_living_entity_use_item_tick`.
    pub(in crate::game) fn native_living_entity_use_item_stop(
        &mut self,
        stack: &ItemStack,
        entity: Option<i32>,
        hand: Hand,
        duration: i32,
        canceled: &mut bool,
    ) {
        self.fire_living_entity_use_item_stop(stack, entity, hand, duration, canceled);
    }
    /// `Item.releaseUsing` of `stack`: `entity` let it go with `time_left`
    /// ticks of its `native_use_duration` left, so it was held for
    /// `duration - time_left` (a bow-like item shoots by that charge). Set
    /// `used` as vanilla's return, and the stack's use cooldown starts
    /// (`ItemStack.releaseUsing`). After the vanilla item's own release, once
    /// `native_living_entity_use_item_stop` let it through. Both sides, as
    /// vanilla's: change the world only where this `Game` runs the world,
    /// not on a server's client.
    pub(in crate::game) fn native_release_using(
        &mut self,
        stack: &ItemStack,
        entity: Option<i32>,
        time_left: i32,
        used: &mut bool,
    ) {
        self.fire_release_using(stack, entity, time_left, used);
    }
    /// A player right clicks a block (NeoForge's
    /// `PlayerInteractEvent.RightClickBlock`): once per hand, where
    /// `useItemOn` starts, on the clicking client and on the server (`click`
    /// says which, with the block, the hit, the hand and its stack, and the
    /// player). Before the block's own use (a chest, a door, a lever) and the
    /// item's (`Item.useOn`, `native_use_on`).
    ///
    /// `use_block`: False skips; True overrides sneak; Default skips when sneaking with either hand full (the both-stacks sneak-bypass exception remains open in #2934). `use_item`: False skips; True forces
    /// even an empty hand or cooldown; Default requires a held stack off cooldown. Empty hands reach
    /// ItemAfterBlock with no stack, but no item Part. Adventure restrictions still apply; block consumption ends the click. Cancelling
    /// skips both and answers `cancellation_result` in their place:
    /// `Pass` (the default) goes on to the item's own use (`Item.use`) and
    /// then the other hand, `Fail` ends the click, and `Success`,
    /// `SuccessServer` or `Consume` end it as used, with the swing they name.
    /// A mod that keeps players out of land cancels with `Fail`.
    pub(in crate::game) fn native_right_click_block(
        &mut self,
        click: &BlockClick<'_>,
        use_block: &mut TriState,
        use_item: &mut TriState,
        cancellation_result: &mut mod_api::InteractionResult,
        canceled: &mut bool,
    ) {
        self.fire_right_click_block(click, use_block, use_item, cancellation_result, canceled);
    }
    /// Whether a mod listens to `native_entity_tick_pre` and `native_entity_tick_post`: a mod
    /// that hooks them hooks this one too and sets `listened` (HEAD, `*listened = true`).
    /// Without it the game tells no entity's tick, and keeps no list of them. Asked once for
    /// each family each tick, on the server and on a client.
    pub(in crate::game) fn native_entity_tick_listened(&mut self, listened: &mut bool) {
        self.fire_entity_tick_listened(listened);
    }
    /// The entities `entities` are about to tick in the level `dimension`, and `kinds` holds
    /// the type of each, in the same order (NeoForge's `EntityTickEvent.Pre`, right before each
    /// entity's tick in `tickNonPassenger`). Entity families may be observed on either side;
    /// see the hook catalog for coverage. Some routes run one call per entity, while others
    /// batch the family. Only IDs offered to this call may enter its cancellation output.
    pub(in crate::game) fn native_entity_tick_pre(
        &mut self,
        dimension: &str,
        entities: &[u32],
        kinds: &[&str],
        canceled: &mut Vec<u32>,
    ) {
        self.fire_entity_tick_pre(dimension, entities, kinds, canceled);
    }
    /// The entities `entities` ticked in the level `dimension`, and `kinds` holds the type of
    /// each, in the same order (NeoForge's `EntityTickEvent.Post`): those
    /// `native_entity_tick_pre` was told and did not cancel, also one that died or was removed in
    /// its tick. An entity told alone in its Pre is told alone here, right after its own tick; a
    /// family once, after its last tick. On the server and on a client.
    pub(in crate::game) fn native_entity_tick_post(
        &mut self,
        dimension: &str,
        entities: &[u32],
        kinds: &[&str],
    ) {
        self.fire_entity_tick_post(dimension, entities, kinds);
    }
    /// LivingEntity.knockback before resistance and motion, after the unchanged v1 hook.
    /// Cancellation from either hook wins. Attacker is the attacking entity's network id,
    /// falling back to the victim's last hurt-by mob, or 0 when unknown; `attacker_main` and
    /// `attacker_off` are what it holds (a player's selected and off-hand stacks, a mob's main hand), none when unknown. Player attackers
    /// include the guest whose action runs. Includes mob melee and armor-stand sweeps, not
    /// direct velocity pushes (explosions, Punch arrows, hoglin throws). Non-finite changes
    /// are restored before resistance. Server-side, without self: damage borrows the victim.
    /// Whether a mod hears projectiles' impacts (`native_projectile_impact`):
    /// set `hears`. Asked once a server tick before the projectiles fly; only
    /// then does the tick keep the copy of each flying projectile that a
    /// canceled impact flies on from, so a game without such a mod pays
    /// nothing. Server-side.
    pub(in crate::game) fn native_hears_projectile_impacts(&self, hears: &mut bool) {
        self.fire_hears_projectile_impacts(hears);
    }
}

/// Register namespaced NBT component types before content loads.
pub(in crate::game) fn native_component_types(
    context: &mod_api::RegistrationContext,
    types: &mut Vec<String>,
) {
    crate::game::native_hooks::fire::fire_component_types(context, types);
}

/// Collect blocks from native mods once at startup on each side, before resources load.
/// Each declaration belongs to the submitting package and joins the content declarations
/// used by saves and the join check. The event's `register` call returns a typed error for
/// invalid IDs or fields. A completed listener's refusals stop startup even if ignored;
/// a panicking listener's batch and diagnostics are discarded.
pub(in crate::game) fn native_blocks(
    context: &mod_api::RegistrationContext,
    blocks: &mut mod_api::__private::DeclarationStage<'_, mod_api::Block>,
) {
    crate::game::native_hooks::fire::fire_blocks(context, blocks);
}

/// Collect items from native mods at the same startup cutoff as `native_blocks`,
/// with the same ownership and refusal rules. A block's item is declared with its block,
/// not here.
pub(in crate::game) fn native_items(
    context: &mod_api::RegistrationContext,
    items: &mut mod_api::__private::DeclarationStage<'_, mod_api::Item>,
) {
    crate::game::native_hooks::fire::fire_items(context, items);
}

/// Standalone files and bakers, registered before resource loading.
/// Every resource load rebakes them; dispatch attributes each entry to its owning mod.
pub(in crate::game) fn native_standalone_models(
    context: &mod_api::RegistrationContext,
    registrations: &mut Vec<Answered<StandaloneModelRegistration>>,
) {
    crate::game::native_hooks::fire::fire_standalone_models(context, registrations);
}

/// What clients get of the block entity at `pos` when its `id` is
/// yours (`BlockEntity.getUpdateTag`): set `tag` to it, read from
/// `entity` alone. Asked on the server whenever a chunk holding it is
/// sent and whenever its block changes, on the server thread; a client's copy of the block
/// entity (made by `native_block_entity` there) merges it into its `nbt`.
/// Leave it unset to send no tag of yours. A block change puts the one-shot fields of
/// `BlockEntityView::set_client_update_fields` over it, and sends them alone when it is unset.
pub(in crate::game) fn native_block_entity_update_tag(
    pos: [i32; 3],
    entity: &ModBlockEntity,
    tag: &mut Option<Compound>,
) {
    crate::game::native_hooks::fire::fire_block_entity_update_tag(pos, entity, tag);
}

/// A player pressed a mod menu's button (`clickMenuButton`), on the
/// server. `subject` lends sequential access to the live resident block
/// entity (when present) and the server `Host`; blockless menus lend no
/// view while keeping `host()` and `transaction(...)` usable. Mods may
/// change the block entity in place and the clicking player's carried stack.
pub(in crate::game) fn native_menu_button(
    menu: &str,
    block: Option<&str>,
    context: &mod_api::item::menu::MenuButtonContext,
    subject: mod_api::BlockEntityScope<'_>,
    carried: &mut Option<mod_api::ItemStack>,
) {
    crate::game::native_hooks::fire::fire_menu_button(menu, block, context, subject, carried);
}

/// The accepted menu lends its resident block entity and asks the Parts to route the move.
pub(in crate::game) fn native_menu_quick_move(
    kind: mod_api::BlockEntityTypeKey,
    subject: mod_api::BlockEntityScope<'_>,
    container_slots: usize,
    slot: usize,
    stack: &mut Option<mod_api::ItemStack>,
    handled: &mut bool,
) -> bool {
    let mut panicked = false;
    crate::game::native_hooks::fire::fire_menu_quick_move(
        kind,
        subject,
        container_slots,
        slot,
        stack,
        handled,
        &mut panicked,
    );
    panicked
}

impl HitLevel<'_> {
    /// NeoForge's `LivingIncomingDamageEvent`: `LivingEntity.hurtServer`, past its guards (an
    /// invulnerable or dead entity, fire on a fire-resistant one), is about to hurt entity
    /// `entity` of type `kind` (`minecraft:zombie`, `minecraft:player`) by `source`: its damage
    /// type (`minecraft:fall`), causing entity (`DamageSource.getEntity`: the shooter of an
    /// arrow), direct entity (`getDirectEntity`: the projectile's entity id when the
    /// damage type names one, on a player, mob or ender dragon; 0 for `magic()` and any
    /// other source with no direct entity, even during that projectile's impact), and
    /// where it comes from (`getSourcePosition`). `victim` is where the entity stands and
    /// looks, and `weapon` the direct entity's weapon (`getWeaponItem`) when it is known.
    /// `causing_weapon` is the causing living entity's current main-hand weapon
    /// (`getEntity().getWeaponItem`), read at hit time, not a projectile's saved
    /// weapon; None for an absent, unknown or non-living cause, or once the shooter
    /// is gone. It is an addition: NeoForge's `LivingIncomingDamageEvent` doesn't
    /// carry it, and mods there read the source entity's main hand themselves.
    /// `container` holds the damage before any reduction: change its `new_damage`, or the
    /// invulnerability ticks a fresh hit leaves (vanilla's 20), or set `canceled` and nothing
    /// of `hurtServer` happens (no damage, hurt sound, knockback, cooldown, statistics or
    /// criteria); a canceled hit on the ender dragon still counts as landed (`EnderDragon.hurt`
    /// returns true whatever `reallyHurt` did). It fires on every damage route, for players,
    /// mobs and the ender dragon. The level is the hit's: its blocks, random and own
    /// attachments; the hurt entity is in the middle of its hit.
    pub(in crate::game) fn native_living_incoming_damage(
        &mut self,
        call: mod_api::__private::LivingIncomingDamageCall<'_>,
    ) {
        self.fire_living_incoming_damage(call);
    }
    /// NeoForge's `LivingKnockBackEvent`: `LivingEntity.knockback` is about to push entity
    /// `entity` of type `kind` (`minecraft:zombie`, `minecraft:player`) `strength` away from the
    /// direction (`x`, `z`), on the raw strength, before the entity's knockback resistance takes
    /// its share and its motion changes. Vanilla's direction points at the source of a hit (the
    /// attacker's side), and the entity is pushed the other way. `source` is the push's
    /// `DamageSource`, named as `native_living_incoming_damage` names a hit's. `attacker` is the
    /// attacking entity's network id, falling back to the victim's last hurt-by mob, or 0 when
    /// unknown; `attacker_main` and `attacker_off` are what it holds (a player's selected and
    /// off-hand stacks, a mob's main hand), none when unknown. Change `strength`, `x` or `z`, or
    /// set `canceled`, and the push follows them: canceled, the entity's motion is left alone. A
    /// `strength` of 0 or less after the hook is no push, and a direction too short to tell
    /// draws a random one after it, as in vanilla. A value a mod leaves that is not a finite
    /// number is put back as it was. It fires for the players, mobs and armor stands the game
    /// knocks back: a hit's own push, a sprinting or Knockback hit, a sweep, a spear, a
    /// shield's push on its holder, a goat's ram and a mob's melee push. It does not fire for a
    /// sulfur cube with a block that a hit with a causing entity knocks back
    /// (`SulfurCube.knockback` does not call `super` there), for the pushes vanilla doesn't make
    /// with `LivingEntity.knockback` (the ender dragon's wings, Punch arrows, explosions, wind
    /// charges, a mace smash's area, a hoglin's throw), nor for the ones Sunrise makes another
    /// way (the ender dragon's). The level is the hit's: its blocks, random and own attachments.
    pub(in crate::game) fn native_living_knockback(
        &mut self,
        call: mod_api::__private::LivingKnockbackCall<'_>,
    ) {
        self.fire_living_knockback(call);
    }
}

/// Server-level helpers for native mods.
impl GameLevel<'_> {
    /// Sets property `key` of the block at `pos` to `value` with
    /// `Level.setBlock(pos, state, flags)`, from a hook that gets the level's
    /// `ticks` (`native_random_tick`, `native_tick_block`). A crop's random
    /// tick grows it with `flags::UPDATE_CLIENTS`: neighbours' shapes update,
    /// but no neighbour gets `neighborChanged` and comparators aren't told;
    /// `flags::UPDATE_ALL` does both. `UPDATE_CLIENTS` sends a block update;
    /// `UPDATE_INVISIBLE` suppresses its notification only on clients, as in
    /// `Level.setBlock`. Any loaded position, not only
    /// the hook's; true when the block changed, false off an authoritative
    /// server, when the block has no such property or value, `pos` is not
    /// loaded, or the property already has `value` (no write, as
    /// `Level.setBlock` returns false for an unchanged state). Elsewhere,
    /// `Game::native_set_block_property_with` does the same.
    pub(in crate::game) fn native_set_block_property(
        &mut self,
        ticks: &mut Ticks,
        pos: [i32; 3],
        key: &str,
        value: &str,
        flags: i32,
    ) -> bool {
        if !self.native_authoritative {
            return false;
        }
        let at = BlockPos::new(pos[0], pos[1], pos[2]);
        let state = self.block_state(at);
        let Some(id) = property_change(self.registry, state, key, value) else {
            return false;
        };
        let b = self.behaviours;
        b.set_block(self, ticks, at, id, flags)
    }
}

/// Helpers for native mods, kept as stable as the hook points.
// Native mods use these; nothing in Sunrise does.
impl Game<'_> {
    /// The server-owned holders' attachments; `TYPE.get(host, holder)` reads
    /// one. On a client it is the copy of what the server synced: `set` and
    /// `remove` change only the copy.
    pub(in crate::game) fn attachments(&mut self) -> &mut dyn AttachmentStore {
        self
    }
    /// Read a loaded, light-emitting block. No implicit chunk loading.
    pub(in crate::game) fn lit_block_name(&self, pos: [i32; 3]) -> Option<mod_api::BlockKey> {
        self.world
            .chunks
            .contains_key(&(pos[0].div_euclid(16), pos[2].div_euclid(16)))
            .then(|| {
                self.res
                    .facts()
                    .get(self.world.block(pos[0], pos[1], pos[2]))
            })
            .filter(|f| f.emission > 0)
            .and_then(|f| mod_api::__private::block_key(&f.name))
    }
    /// Whether the player left mod `mod_id` on in Options > Mods. A mod
    /// turned off does nothing (check this first in each hook) until the
    /// launcher's next build leaves it out.
    pub(in crate::game) fn on(&self, mod_id: &str) -> bool {
        !self
            .mods
            .code_disabled(&self.settings.mods.disabled, mod_id)
    }
    /// The most of the item `item` one slot holds (`Item.getDefaultMaxStackSize`),
    /// or `None` for an unknown item: for a mod's container that fills or
    /// merges stacks itself.
    pub(in crate::game) fn max_stack_size(&self, item: mod_api::ItemKey) -> Option<u32> {
        self.res
            .items()
            .get(mod_api::__private::item_key_id(item)?)
            .map(|i| i.max_stack)
    }
    pub(in crate::game) fn max_damage(&self, item: mod_api::ItemKey) -> Option<u32> {
        self.res
            .items()
            .get(mod_api::__private::item_key_id(item)?)
            .map(|i| i.max_damage)
    }
    pub(in crate::game) fn item_key(&self, id: &mod_api::Identifier) -> Option<mod_api::ItemKey> {
        mod_api::__private::item_key_by_identifier(id)
    }
    /// The clock of the world this window shows, for client mods: its game
    /// time and time of day as the server last sent them
    /// (`ClientboundSetTimePacket`), advanced each tick between packets; none
    /// on the title screen.
    pub(in crate::game) fn clock(&self) -> Option<mod_api::Clock> {
        self.player.is_some().then_some(mod_api::Clock {
            game_time: self.world.game_time,
            day_time: self.world.day_time,
        })
    }
    /// The instance (game) folder, which its players share: `saves/`,
    /// `resourcepacks/`, `shaderpacks/` and `sunrise-mods/` are in it, and the
    /// title's screens create worlds and read packs there. A world's own
    /// `datapacks/` is in that world's folder (`saves/<world>/datapacks`).
    /// The folder the game runs in (`--player-dir` moves a player's options
    /// and servers only, not the instance). Nothing is created.
    pub(in crate::game) fn instance_dir(&self) -> PathBuf {
        crate::game::game_dir()
    }
    /// A mod's setting `key` (one file per key in `native_data_dir`). A mod's hook or part reads
    /// only its own mod's: another mod's id gives None.
    pub(in crate::game) fn setting(&self, mod_id: &str, key: &str) -> Option<String> {
        if crate::game::another_mods(mod_id) {
            return None;
        }
        self.stored_setting(mod_id, key)
    }
    pub(in crate::game) fn stored_setting(&self, mod_id: &str, key: &str) -> Option<String> {
        mod_settings::read_raw(&self.native_data_dir(mod_id), mod_id, key)
    }
    /// Keeps a mod's setting `key`. A mod's hook or part keeps only its own mod's.
    pub(in crate::game) fn set_setting(&self, mod_id: &str, key: &str, value: &str) {
        if crate::game::another_mods(mod_id) {
            return;
        }
        self.store_setting(mod_id, key, value);
    }
    pub(in crate::game) fn store_setting(&self, mod_id: &str, key: &str, value: &str) -> bool {
        mod_settings::store(&self.native_data_dir(mod_id), mod_id, key, value)
    }
    /// The recipes of type `kind` (`minecraft:smelting`, `mymod:crusher`), vanilla's
    /// types and mods' own alike, from `data/<namespace>/recipe/*.json` of vanilla,
    /// the content mods (`.smod`) and the world's data packs, in file order: each
    /// one typed (`Recipe`: its ingredients, results, group, cooking time and
    /// experience, or its stonecutting, smithing or special crafting rule),
    /// with its file as JSON for a mod's own type. Dynamic rules are not fixed
    /// outputs; use `LevelRef::craft` to assemble a crafting grid. These are the
    /// recipes in force (`RecipeManager.getAllRecipesFor`): data packs merged and
    /// load conditions held, so a pack's override or removal shows. A data pack
    /// changes them on `/reload`, so look them up again when you need them. Empty
    /// for a type nobody uses and outside a world. These are the server's table on
    /// the server thread, a dedicated server or a LAN host; a client, the
    /// single-player window included, sees vanilla's and the content mods' recipes,
    /// not the world's data packs', unless a mod had the server send it the type
    /// (`native_datapack_sync`).
    pub(in crate::game) fn recipes_of_type(
        &self,
        kind: &str,
    ) -> crate::game::api_values::RecipeListing {
        if self.outside_world() {
            return Default::default();
        }
        crate::game::api_values::RecipeListing(
            self.recipes()
                .by_type
                .get(kind)
                .cloned()
                .unwrap_or_default(),
        )
    }
    /// An ingredient as a recipe's JSON names it: an item, a `#tag`, or a list
    /// of them, a bare id as `minecraft:` (as vanilla reads it); `test` says
    /// whether a stack fits. Items the game doesn't know are left out; None
    /// for missing tags, invalid inputs or when no named item is known, and
    /// outside a world. Existing-empty tags retain an empty ingredient.
    pub(in crate::game) fn recipe_ingredient(
        &self,
        json: &str,
    ) -> Result<Option<Ingredient>, mod_api::RecipeJsonError> {
        if self.outside_world() {
            return Ok(None);
        }
        let json: serde_json::Value =
            serde_json::from_str(json).map_err(|_| mod_api::RecipeJsonError::InvalidJson)?;
        let qualify = |s: &str| match s.strip_prefix('#') {
            Some(tag) => String::from("#") + &qualified(tag),
            None => qualified(s),
        };
        let json = match &json {
            serde_json::Value::String(s) => serde_json::Value::String(qualify(s)),
            serde_json::Value::Array(a) => serde_json::Value::Array(
                a.iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(|s| serde_json::Value::String(qualify(s)))
                    .collect(),
            ),
            _ => return Ok(None),
        };
        let items = self.items();
        let Some(mut ingredient) = sunrise_client::crafting::Ingredient::parse(&json, items) else {
            return Ok(None);
        };
        let known: Vec<String> = std::mem::take(&mut ingredient.order)
            .into_iter()
            .filter(|item| items.get(item).is_some())
            .collect();
        Ok((ingredient.items.is_empty() || !known.is_empty()).then(|| Ingredient::of(known)))
    }
    /// A stack as a recipe's `result` names it: `id` (a bare one as
    /// `minecraft:`), `count` (1 by default, up to 99) and `components`. None
    /// without an `id`, for an item the game doesn't know or a count outside
    /// that, and outside a world.
    pub(in crate::game) fn recipe_result(
        &self,
        json: &str,
    ) -> Result<Option<ItemStack>, mod_api::RecipeJsonError> {
        if self.outside_world() {
            return Ok(None);
        }
        let mut json: serde_json::Value =
            serde_json::from_str(json).map_err(|_| mod_api::RecipeJsonError::InvalidJson)?;
        if let Some(id) = json.as_str() {
            json = qualified(id).into();
        } else if let Some(id) = json["id"].as_str() {
            json["id"] = qualified(id).into();
        }
        Ok(recipe_result(&json).filter(|s| self.items().get(&s.item).is_some()))
    }
}

pub(in crate::game) fn startup_setting(mod_id: &str, key: &str) -> Option<String> {
    if crate::game::another_mods(mod_id) {
        return None;
    }
    mod_settings::read_raw(&crate::game::mod_data_root().join(mod_id), mod_id, key)
}

/// A mod's hook or part is running, and `mod_id` is another mod's.
pub(in crate::game) fn another_mods(mod_id: &str) -> bool {
    mod_api::content::crash::current_native_mod().is_some_and(|running| running != mod_id)
}

/// The level a placement or survival question is asked of: the game's level with the
/// simulation's ticks shared beside it, which a signal read needs (comparator outputs, moving
/// pistons). The listener's `&dyn LevelHost` is this, so `state_for_placement`, `can_survive`,
/// `may_place_on` and `can_sustain_plant` can read redstone and shapes; asked from a preview
/// (a client's guess at a click), it reads over the blocks the preview set. A read goes only
/// through `Signals`, which asks the signal hooks and never `state_for_placement` or
/// `can_survive`: a listener cannot re-enter itself through it.
pub(in crate::game) type LevelView<'a> =
    super::super::mod_api_host::LevelTicks<'a, &'a GameLevel<'a>, &'a Ticks>;
impl GameLevel<'_> {
    /// This level as a placement or survival question sees it, with the simulation's `ticks`.
    pub(in crate::game) fn view<'a>(&'a self, ticks: &'a Ticks) -> LevelView<'a> {
        super::super::mod_api_host::LevelTicks::new(self, ticks)
    }
}
impl LevelView<'_> {
    /// `VegetationBlock.mayPlaceOn` for the plant `state` on `ground` at
    /// `ground_pos`, when the soil's `can_sustain_plant` leaves the decision to
    /// the plant. The plant inherits vegetation rules from its template or opts
    /// into them in its block declaration. Answer only for your own blocks.
    /// The level is read-only. Asked on both sides: a client asks it from its
    /// placement prediction and its own shape updates.
    #[inline(always)]
    pub(in crate::game) fn native_may_place_on(
        &self,
        dimension: LevelDimension,
        state: BlockStateId,
        ground: BlockStateId,
        ground_pos: BlockPos,
        may: &mut Option<bool>,
        inert: &mut bool,
    ) {
        self.fire_may_place_on(dimension, state, ground, ground_pos, may, inert);
    }

    /// `BlockState.canSurvive` for `state` at `pos`
    /// (`BlockBehaviour.canSurvive`), before the block answers it: as it is
    /// placed (`BlockItem.canPlace`), from a shape update of a block whose
    /// template checks it (a plant, a torch, a sign: false breaks the block with
    /// its drops), and wherever else vanilla asks. `state` may not be in the
    /// world yet, so decide from `state`. Answer only for your own blocks:
    /// `Some(false)` refuses the spot, `Some(true)` allows it, and the block's
    /// own rule doesn't run. The level is read-only. Asked on every shape update
    /// of such a block: keep it short. Asked on both sides: a client asks it from
    /// its placement prediction and its own shape updates.
    #[inline(always)]
    pub(in crate::game) fn native_can_survive(
        &self,
        dimension: LevelDimension,
        pos: BlockPos,
        state: BlockStateId,
        survives: &mut Option<bool>,
        inert: &mut bool,
    ) {
        self.fire_can_survive(dimension, pos, state, survives, inert);
    }

    /// A block is about to take the state it is placed in
    /// (`Block.getStateForPlacement(BlockPlaceContext)`), `state` the block's
    /// own default state. Answer only for your blocks: put the state you want
    /// in `answer` as `Some(Some(state))`, including a registered state of
    /// another block, or refuse with `Some(None)`: the block goes nowhere
    /// and the stack is kept, as vanilla's `null`. Left empty, the block's
    /// own placement runs. `context` is vanilla's `BlockPlaceContext` (the
    /// clicked face, the clicked position, `source` distinguishing `Player`
    /// from `Directional`, whether the clicked block is being replaced, the
    /// item in hand) and `fluid` the fluid in the position the block would go
    /// in, which a waterlogged block reads. Build the state from `state` with
    /// `native_state_with_property`, and know whether `state` is yours with
    /// `native_state_block`. It is asked for every placer that goes
    /// through vanilla's placement (a player's `BlockItem.place`, a
    /// dispenser's `DirectionalPlaceContext`), and the answer still takes its
    /// survival and entity obstruction checks. A trait mod returns
    /// `Some(Some(state))` to choose a state, `Some(None)` to refuse, or
    /// `None` to leave the answer to the next mod or vanilla. Asked on both
    /// sides: a client asks it from its placement prediction.
    #[inline(always)]
    #[allow(clippy::option_option)]
    pub(in crate::game) fn native_state_for_placement(
        &self,
        dimension: LevelDimension,
        state: BlockStateId,
        context: &PlacementContext,
        fluid: mod_api::FluidState,
        answer: &mut Option<Option<BlockStateId>>,
    ) {
        self.fire_state_for_placement(dimension, state, context, fluid, answer);
    }
}

#[cfg(test)]
#[path = "base_tests/screen_open_tests.rs"]
pub(in crate::game) mod screen_open_tests;

#[cfg(test)]
#[path = "base_tests/screen_widgets_tests.rs"]
pub(in crate::game) mod screen_widgets_tests;

#[cfg(test)]
pub(in crate::game) mod text_tests {
    use super::*;

    /// `DrawContext::text` takes ARGB as vanilla's `GuiGraphics` does: the alpha reaches the
    /// vertices, and fully transparent text draws nothing.
    #[test]
    #[ignore = "daily"]
    fn text_takes_argb_and_skips_transparent_text() {
        use mod_api::DrawContext;
        crate::game::native_entity_refs::tests::with_client_game(|game| {
            let drawn = |argb: u32| {
                let mut batch = HudBatch::scratch([640, 480]);
                crate::game::host::draw::GameDrawContext::new(game, &mut batch).text(
                    "Hi",
                    [4.0, 4.0],
                    argb,
                    false,
                );
                batch.vertices
            };
            let opaque = drawn(0xFFFF_FFFF);
            let half = drawn(0x80FF_FFFF);
            assert!(!opaque.is_empty());
            assert_eq!(opaque.len(), half.len());
            assert!(opaque.iter().all(|v| v.color >> 24 == 0xFF));
            assert!(half.iter().all(|v| v.color >> 24 == 0x80));
            // RGB alone, as the API took before, is fully transparent now.
            assert!(drawn(0x00FF_FFFF).is_empty());
        });
    }
}

#[cfg(test)]
pub(in crate::game) mod held_break_slot_tests {}
