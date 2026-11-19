package dev.sunriselab.parity;

import com.google.gson.*;
import java.lang.reflect.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.*;
import net.minecraft.commands.arguments.blocks.BlockStateParser;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.core.particles.ParticleOptions;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.sounds.SoundEvent;
import net.minecraft.sounds.SoundSource;
import net.minecraft.util.RandomSource;
import net.minecraft.world.entity.EntityDimensions;
import net.minecraft.world.entity.HumanoidArm;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.GameType;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.levelgen.LegacyRandomSource;
import net.minecraft.world.level.block.EntityBlock;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.entity.SpawnerBlockEntity;
import net.minecraft.world.level.block.entity.TrialSpawnerBlockEntity;
import net.minecraft.world.level.block.entity.vault.VaultBlockEntity;
import net.minecraft.world.level.block.entity.vault.VaultSharedData;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.BlockStateProperties;
import net.minecraft.world.phys.Vec3;

/**
 * Original oracle for the client ticks of spawners, trial spawners and vaults
 * (BaseSpawner.clientTick, TrialSpawner.tickClient, VaultBlockEntity.Client.tick) as
 * ClientLevel.tickBlockEntities runs them: every loaded one, each client tick, in the level's
 * ticker order, all drawing from the one level random. A case places its block entities in order
 * (each ticker goes to the end of the list); unloading a chunk drops its tickers, and loading it
 * again makes its block entities anew (as a chunk packet does) and adds their tickers in the
 * order LevelChunk.registerAllBlockEntitiesAfterLevelLoad walks its Object2ObjectOpenHashMap.
 *
 * The level is allocated without Level's constructor and answers only what these ticks ask:
 * its random, game time, players, particles and local sounds. Spawners get a display entity
 * first, as SpawnerRenderer makes one (BaseSpawner.clientTick does nothing without it).
 *
 * Input lines: case JSON. Output lines: {ticks: [{random, events, spins}]} or {error}. Doubles and
 * floats are their raw bits, as hex strings.
 */
public final class VanillaBlockEntityClientTickOracle {
    private static Object unsafe;
    private static Method allocate;

    @SuppressWarnings("unchecked")
    static <T> T allocate(Class<T> type) throws Exception {
        return (T) allocate.invoke(unsafe, type);
    }

    static void set(Class<?> owner, Object target, String name, Object value) throws Exception {
        Field field = owner.getDeclaredField(name);
        field.setAccessible(true);
        field.set(target, value);
    }

    static String bits(double d) { return Long.toHexString(Double.doubleToRawLongBits(d)); }

    static String bits(float f) { return Integer.toHexString(Float.floatToRawIntBits(f)); }

    /** The player these ticks look for: alive or not, a spectator or not, standing. */
    static final class CasePlayer extends Player {
        boolean dead, spectator;
        CasePlayer() { super(null, null); }
        @Override public GameType gameMode() { return spectator ? GameType.SPECTATOR : GameType.SURVIVAL; }
        @Override public HumanoidArm getMainArm() { return HumanoidArm.RIGHT; }
        @Override public boolean isSpectator() { return spectator; }
        @Override public boolean isAlive() { return !dead; }
    }

    /** The client level as these ticks see it. */
    static final class CaseLevel extends net.minecraft.world.level.Level {
        RandomSource random;
        long gameTime;
        List<Player> players;
        JsonArray events;
        CaseLevel() { super(null, null, null, null, false, false, 0L, 0); }
        @Override public RandomSource getRandom() { return random; }
        @Override public long getGameTime() { return gameTime; }
        @Override public java.util.List players() { return players; }
        @Override public Player getPlayerByUUID(UUID uuid) {
            for (Player p : players) if (p.getUUID().equals(uuid)) return p;
            return null;
        }
        void particle(ParticleOptions options, double x, double y, double z, double vx, double vy, double vz) {
            var e = new JsonArray();
            e.add("particle");
            e.add(BuiltInRegistries.PARTICLE_TYPE.getKey(options.getType()).toString());
            for (double d : new double[] {x, y, z, vx, vy, vz}) e.add(bits(d));
            events.add(e);
        }
        @Override public void addParticle(ParticleOptions o, double x, double y, double z, double vx, double vy, double vz) { particle(o, x, y, z, vx, vy, vz); }
        @Override public void addParticle(ParticleOptions o, boolean a, boolean b, double x, double y, double z, double vx, double vy, double vz) { particle(o, x, y, z, vx, vy, vz); }
        @Override public void addAlwaysVisibleParticle(ParticleOptions o, double x, double y, double z, double vx, double vy, double vz) { particle(o, x, y, z, vx, vy, vz); }
        @Override public void addAlwaysVisibleParticle(ParticleOptions o, boolean a, double x, double y, double z, double vx, double vy, double vz) { particle(o, x, y, z, vx, vy, vz); }
        // ClientLevel.playLocalSound: the sound's seed is the level random's next long.
        @Override public void playLocalSound(double x, double y, double z, SoundEvent sound, SoundSource source, float volume, float pitch, boolean delay) {
            var e = new JsonArray();
            e.add("sound");
            e.add(sound.location().toString());
            for (double d : new double[] {x, y, z}) e.add(bits(d));
            e.add(bits(volume));
            e.add(bits(pitch));
            random.nextLong();
            events.add(e);
        }
        @Override public BlockState getBlockState(BlockPos p) { throw new UnsupportedOperationException(); }
        @Override public net.minecraft.world.clock.ClockManager clockManager() { throw new UnsupportedOperationException(); }
        @Override public void destroyBlockProgress(int p0, BlockPos p1, int p2) { throw new UnsupportedOperationException(); }
        @Override public java.util.Collection dragonParts() { throw new UnsupportedOperationException(); }
        @Override public net.minecraft.world.flag.FeatureFlagSet enabledFeatures() { throw new UnsupportedOperationException(); }
        @Override public net.minecraft.world.attribute.EnvironmentAttributeSystem environmentAttributes() { throw new UnsupportedOperationException(); }
        @Override public void explode(net.minecraft.world.entity.Entity p0, net.minecraft.world.damagesource.DamageSource p1, net.minecraft.world.level.ExplosionDamageCalculator p2, double p3, double p4, double p5, float p6, boolean p7, net.minecraft.world.level.Level.ExplosionInteraction p8, ParticleOptions p9, ParticleOptions p10, net.minecraft.util.random.WeightedList p11, Holder p12) { throw new UnsupportedOperationException(); }
        @Override public void gameEvent(Holder p0, Vec3 p1, net.minecraft.world.level.gameevent.GameEvent.Context p2) { throw new UnsupportedOperationException(); }
        @Override public String gatherChunkSourceStats() { throw new UnsupportedOperationException(); }
        @Override public net.minecraft.world.ticks.LevelTickAccess getBlockTicks() { throw new UnsupportedOperationException(); }
        @Override public net.minecraft.world.level.chunk.ChunkSource getChunkSource() { throw new UnsupportedOperationException(); }
        @Override public net.minecraft.world.level.entity.LevelEntityGetter getEntities() { throw new UnsupportedOperationException(); }
        @Override public net.minecraft.world.entity.Entity getEntity(int p0) { throw new UnsupportedOperationException(); }
        @Override public net.minecraft.world.ticks.LevelTickAccess getFluidTicks() { throw new UnsupportedOperationException(); }
        @Override public net.minecraft.world.level.saveddata.maps.MapItemSavedData getMapData(net.minecraft.world.level.saveddata.maps.MapId p0) { throw new UnsupportedOperationException(); }
        @Override public net.minecraft.world.level.storage.LevelData.RespawnData getRespawnData() { throw new UnsupportedOperationException(); }
        @Override public net.minecraft.world.scores.Scoreboard getScoreboard() { throw new UnsupportedOperationException(); }
        @Override public int getSeaLevel() { throw new UnsupportedOperationException(); }
        @Override public Holder getUncachedNoiseBiome(int p0, int p1, int p2) { throw new UnsupportedOperationException(); }
        @Override public net.minecraft.world.level.border.WorldBorder getWorldBorder() { throw new UnsupportedOperationException(); }
        @Override public void levelEvent(net.minecraft.world.entity.Entity p0, int p1, BlockPos p2, int p3) { throw new UnsupportedOperationException(); }
        @Override public void playSeededSound(net.minecraft.world.entity.Entity p0, net.minecraft.world.entity.Entity p1, Holder p2, SoundSource p3, float p4, float p5, long p6) { throw new UnsupportedOperationException(); }
        @Override public void playSeededSound(net.minecraft.world.entity.Entity p0, double p1, double p2, double p3, Holder p4, SoundSource p5, float p6, float p7, long p8) { throw new UnsupportedOperationException(); }
        @Override public net.minecraft.world.item.crafting.RecipeAccess recipeAccess() { throw new UnsupportedOperationException(); }
        @Override public void sendBlockUpdated(BlockPos p0, BlockState p1, BlockState p2, int p3) { throw new UnsupportedOperationException(); }
        @Override public void setRespawnData(net.minecraft.world.level.storage.LevelData.RespawnData p0) { throw new UnsupportedOperationException(); }
        @Override public net.minecraft.world.TickRateManager tickRateManager() { throw new UnsupportedOperationException(); }
    }

    static BlockPos pos(JsonArray a) { return new BlockPos(a.get(0).getAsInt(), a.get(1).getAsInt(), a.get(2).getAsInt()); }

    /** The block state of one placed block entity. */
    static BlockState state(JsonObject e) throws Exception {
        String spec = switch (e.get("kind").getAsString()) {
            case "spawner" -> "minecraft:spawner";
            case "trial_spawner" -> "minecraft:trial_spawner[trial_spawner_state=" + e.get("state").getAsString()
                + ",ominous=" + e.get("ominous").getAsBoolean() + "]";
            case "vault" -> "minecraft:vault[facing=" + e.get("facing").getAsString() + ",ominous="
                + e.get("ominous").getAsBoolean() + ",vault_state=" + e.get("state").getAsString() + "]";
            default -> throw new IllegalArgumentException(e.get("kind").getAsString());
        };
        return BlockStateParser.parseForBlock(BuiltInRegistries.BLOCK, spec, false).blockState();
    }

    /** A block entity as the case's chunk packet carries it. */
    static BlockEntity make(JsonObject e, CaseLevel level, Player player) throws Exception {
        BlockPos p = pos(e.getAsJsonArray("pos"));
        BlockState state = state(e);
        BlockEntity be = ((EntityBlock) state.getBlock()).newBlockEntity(p, state);
        switch (e.get("kind").getAsString()) {
            case "spawner" -> {
                var spawner = ((SpawnerBlockEntity) be).getSpawner();
                var base = net.minecraft.world.level.BaseSpawner.class;
                set(base, spawner, "requiredPlayerRange", e.get("range").getAsInt());
                set(base, spawner, "spawnDelay", e.get("delay").getAsInt());
                // SpawnerRenderer's getOrCreateDisplayEntity: any entity will do, the tick only asks for one.
                set(base, spawner, "displayEntity", allocate(net.minecraft.world.entity.animal.pig.Pig.class));
            }
            case "trial_spawner" -> {
                var data = ((TrialSpawnerBlockEntity) be).getTrialSpawner().getStateData();
                set(data.getClass(), data, "nextMobSpawnsAt", e.get("nextMobSpawnsAt").getAsLong());
            }
            case "vault" -> {
                var shared = ((VaultBlockEntity) be).getSharedData();
                if (e.get("display").getAsBoolean()) shared.setDisplayItem(new ItemStack(Items.DIAMOND));
                Set<UUID> connected = new HashSet<>();
                if (e.get("connected").getAsBoolean()) connected.add(player.getUUID());
                set(VaultSharedData.class, shared, "connectedPlayers", connected);
                set(VaultSharedData.class, shared, "connectedParticlesRange", e.get("particlesRange").getAsDouble());
            }
            default -> throw new IllegalArgumentException();
        }
        return be;
    }

    static void tick(BlockEntity be, CaseLevel level) {
        BlockPos p = be.getBlockPos();
        BlockState state = be.getBlockState();
        if (be instanceof SpawnerBlockEntity s) {
            SpawnerBlockEntity.clientTick(level, p, state, s);
        } else if (be instanceof TrialSpawnerBlockEntity t) {
            t.getTrialSpawner().tickClient(level, p, state.getValue(BlockStateProperties.OMINOUS));
        } else if (be instanceof VaultBlockEntity v) {
            VaultBlockEntity.Client.tick(level, p, state, v.getClientData(), v.getSharedData());
        }
    }

    static JsonArray spin(BlockEntity be) {
        var a = new JsonArray();
        BlockPos p = be.getBlockPos();
        a.add(p.getX()); a.add(p.getY()); a.add(p.getZ());
        if (be instanceof SpawnerBlockEntity s) {
            a.add(bits(s.getSpawner().getSpin())); a.add(bits(s.getSpawner().getOSpin()));
        } else if (be instanceof TrialSpawnerBlockEntity t) {
            var data = t.getTrialSpawner().getStateData();
            a.add(bits(data.getSpin())); a.add(bits(data.getOSpin()));
        } else if (be instanceof VaultBlockEntity v) {
            a.add(bits(v.getClientData().currentSpin())); a.add(bits(v.getClientData().previousSpin()));
        }
        return a;
    }

    public static void main(String[] args) throws Exception {
        ParityData.init();
        var unsafeField = Class.forName("sun.misc.Unsafe").getDeclaredField("theUnsafe");
        unsafeField.setAccessible(true);
        unsafe = unsafeField.get(null);
        allocate = unsafe.getClass().getMethod("allocateInstance", Class.class);
        var seedField = LegacyRandomSource.class.getDeclaredField("seed");
        seedField.setAccessible(true);
        var gson = new Gson();
        var out = new StringBuilder();
        for (String line : Files.readAllLines(Path.of(args[0]))) {
            if (line.isBlank()) continue;
            var c = JsonParser.parseString(line).getAsJsonObject();
            var result = new JsonObject();
            try {
                var level = allocate(CaseLevel.class);
                level.random = new LegacyRandomSource(c.get("seed").getAsLong());
                level.gameTime = c.get("gameTime").getAsLong();
                var pc = c.getAsJsonObject("player");
                var player = allocate(CasePlayer.class);
                var at = pc.getAsJsonArray("pos");
                var position = new Vec3(at.get(0).getAsDouble(), at.get(1).getAsDouble(), at.get(2).getAsDouble());
                var entity = net.minecraft.world.entity.Entity.class;
                set(entity, player, "position", position);
                set(entity, player, "blockPosition", BlockPos.containing(position));
                set(entity, player, "dimensions", EntityDimensions.scalable(0.6F, 1.8F));
                set(entity, player, "uuid", new UUID(0x5355_4e52_4953_45L, 1L));
                player.dead = pc.get("dead").getAsBoolean();
                player.spectator = pc.get("spectator").getAsBoolean();
                level.players = List.of(player);
                // The level's tickers and each chunk's block entities (LevelChunk.blockEntities).
                List<BlockEntity> tickers = new ArrayList<>();
                Map<Long, Map<BlockPos, JsonObject>> chunks = new HashMap<>();
                for (var el : c.getAsJsonArray("entities")) {
                    var e = el.getAsJsonObject();
                    var be = make(e, level, player);
                    tickers.add(be);
                    chunks.computeIfAbsent(ChunkPos.pack(be.getBlockPos()), k -> new LinkedHashMap<>()).put(be.getBlockPos(), e);
                }
                var events = c.has("events") ? c.getAsJsonObject("events") : new JsonObject();
                var ticks = new JsonArray();
                for (int t = 0; t < c.get("ticks").getAsInt(); t++) {
                    if (events.has(Integer.toString(t))) {
                        for (var op : events.getAsJsonArray(Integer.toString(t))) {
                            var o = op.getAsJsonArray();
                            long chunk = ChunkPos.pack(o.get(1).getAsInt(), o.get(2).getAsInt());
                            if (o.get(0).getAsString().equals("unload")) {
                                tickers.removeIf(be -> ChunkPos.pack(be.getBlockPos()) == chunk);
                            } else {
                                // A chunk packet's block entities, in the order the chunk's map walks them
                                // (ChunkAccess.blockEntities is an Object2ObjectOpenHashMap).
                                Map<BlockPos, BlockEntity> loaded = new it.unimi.dsi.fastutil.objects.Object2ObjectOpenHashMap<>();
                                for (var e : chunks.getOrDefault(chunk, Map.of()).values()) {
                                    var be = make(e, level, player);
                                    loaded.put(be.getBlockPos(), be);
                                }
                                tickers.addAll(loaded.values());
                            }
                        }
                    }
                    level.events = new JsonArray();
                    for (var be : tickers) tick(be, level);
                    var spins = new JsonArray();
                    tickers.stream().sorted(Comparator.comparing(BlockEntity::getBlockPos)).forEach(be -> spins.add(spin(be)));
                    var row = new JsonObject();
                    // The level random's 48-bit state after the tick: every draw, sounds' seeds too.
                    row.addProperty("random", Long.toHexString(((java.util.concurrent.atomic.AtomicLong) seedField.get(level.random)).get()));
                    row.add("events", level.events);
                    row.add("spins", spins);
                    ticks.add(row);
                    level.gameTime++;
                }
                result.add("ticks", ticks);
            } catch (Throwable error) {
                var cause = error instanceof InvocationTargetException && error.getCause() != null ? error.getCause() : error;
                var sw = new java.io.StringWriter();
                cause.printStackTrace(new java.io.PrintWriter(sw));
                result.addProperty("error", sw.toString().lines().limit(6).reduce("", (a, b) -> a + b + " | "));
            }
            out.append(gson.toJson(result)).append('\n');
        }
        Files.writeString(Path.of(args[1]), out.toString(), StandardCharsets.UTF_8);
    }
}
