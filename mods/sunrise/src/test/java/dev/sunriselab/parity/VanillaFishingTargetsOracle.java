package dev.sunriselab.parity;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonNull;
import com.google.gson.JsonObject;
import com.mojang.authlib.GameProfile;
import com.mojang.authlib.services.ServicesKeySet;
import com.mojang.serialization.Lifecycle;
import io.netty.channel.embedded.EmbeddedChannel;
import java.lang.reflect.Field;
import java.net.Proxy;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Optional;
import java.util.Set;
import java.util.UUID;
import java.util.function.BooleanSupplier;
import net.minecraft.SharedConstants;
import net.minecraft.SystemReport;
import net.minecraft.commands.Commands;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.core.MappedRegistry;
import net.minecraft.core.Registry;
import net.minecraft.core.SectionPos;
import net.minecraft.core.UUIDUtil;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.network.Connection;
import net.minecraft.network.protocol.PacketFlow;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.Services;
import net.minecraft.server.WorldLoader;
import net.minecraft.server.WorldStem;
import net.minecraft.server.level.ChunkMap;
import net.minecraft.server.level.ClientInformation;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.server.level.Ticket;
import net.minecraft.server.level.TicketType;
import net.minecraft.server.level.progress.LoggingLevelLoadListener;
import net.minecraft.server.network.CommonListenerCookie;
import net.minecraft.server.notifications.EmptyNotificationService;
import net.minecraft.server.notifications.NotificationManager;
import net.minecraft.server.packs.repository.PackRepository;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.permissions.LevelBasedPermissionSet;
import net.minecraft.server.permissions.PermissionSet;
import net.minecraft.server.players.NameAndId;
import net.minecraft.server.players.PlayerList;
import net.minecraft.server.players.ProfileResolver;
import net.minecraft.server.players.UserNameToIdResolver;
import net.minecraft.util.RandomSource;
import net.minecraft.util.Util;
import net.minecraft.util.datafix.DataFixers;
import net.minecraft.util.debugchart.LocalSampleLogger;
import net.minecraft.util.debugchart.SampleLogger;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.animal.pig.Pig;
import net.minecraft.world.entity.boss.enderdragon.EnderDragon;
import net.minecraft.world.entity.boss.enderdragon.EnderDragonPart;
import net.minecraft.world.entity.decoration.LeashFenceKnotEntity;
import net.minecraft.world.entity.projectile.FishingHook;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.GameType;
import net.minecraft.world.level.LevelSettings;
import net.minecraft.world.level.WorldDataConfiguration;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.dimension.LevelStem;
import net.minecraft.world.level.gameevent.BlockPositionSource;
import net.minecraft.world.level.gameevent.GameEvent;
import net.minecraft.world.level.gameevent.GameEventListener;
import net.minecraft.world.level.gameevent.PositionSource;
import net.minecraft.world.level.gamerules.GameRules;
import net.minecraft.world.level.levelgen.WorldDimensions;
import net.minecraft.world.level.levelgen.WorldGenSettings;
import net.minecraft.world.level.levelgen.WorldOptions;
import net.minecraft.world.level.levelgen.presets.WorldPresets;
import net.minecraft.world.level.storage.LevelDataAndDimensions;
import net.minecraft.world.level.storage.LevelStorageSource;
import net.minecraft.world.level.storage.PrimaryLevelData;
import net.minecraft.world.phys.Vec3;

// Witnesses FishingHook against every pickable non-mob target, End crystal, dragon part, and ItemEntity in 26.3.
public final class VanillaFishingTargetsOracle {
    static final int TICKS = 6;
    static final long HOOK_SEED = 12345L;
    static final long LEVEL_SEED = 67890L;

    static final String[] TARGETS = {
        "item_frame", "item_frame_empty", "item_frame_fixed", "glow_item_frame",
        "painting", "falling_block", "end_crystal", "ender_dragon",
        "armor_stand", "armor_stand_marker", "armor_stand_small", "armor_stand_nogravity",
        "leash_knot", "leash_knot_tied", "cushion", "dropped_item"
    };

    static final class Resolver implements ProfileResolver, UserNameToIdResolver {
        @Override public Optional<GameProfile> fetchByName(String name) { return Optional.empty(); }
        @Override public Optional<GameProfile> fetchById(UUID id) { return Optional.empty(); }
        @Override public void add(NameAndId nameAndId) {}
        @Override public Optional<NameAndId> get(String name) { return Optional.empty(); }
        @Override public Optional<NameAndId> get(UUID id) { return Optional.empty(); }
        @Override public void resolveOfflineUsers(boolean value) {}
        @Override public void save() {}
    }

    static String dBits(double v) {
        return String.format("%016x", Double.doubleToRawLongBits(v));
    }

    static String fBits(float v) {
        return String.format("%08x", Float.floatToRawIntBits(v));
    }

    static JsonArray vecBits(Vec3 v) {
        JsonArray a = new JsonArray();
        a.add(dBits(v.x));
        a.add(dBits(v.y));
        a.add(dBits(v.z));
        return a;
    }

    static JsonArray vecArr(Vec3 v) {
        JsonArray a = new JsonArray();
        a.add(v.x);
        a.add(v.y);
        a.add(v.z);
        return a;
    }

    static Field field(Class<?> cls, String name) throws ReflectiveOperationException {
        Field f = cls.getDeclaredField(name);
        f.setAccessible(true);
        return f;
    }

    static final class Server extends MinecraftServer {
        final Path output;
        final LocalSampleLogger sampleLogger = new LocalSampleLogger(4);
        boolean ran;
        ServerPlayer player;
        EmbeddedChannel channel;
        final List<JsonObject> tickGameEvents = new ArrayList<>();

        Field fEntityRandom;
        Field fCurrentState;

        static Server create(Thread thread, LevelStorageSource.LevelStorageAccess access, PackRepository packs, Path output) throws Exception {
            packs.reload();
            WorldDataConfiguration config = WorldDataConfiguration.DEFAULT;
            LevelSettings settings = new LevelSettings("FishingWitness", GameType.SURVIVAL, LevelSettings.DifficultySettings.DEFAULT, true, config);
            WorldLoader.PackConfig packConfig = new WorldLoader.PackConfig(packs, config, false, true);
            WorldLoader.InitConfig init = new WorldLoader.InitConfig(packConfig, Commands.CommandSelection.INTEGRATED, LevelBasedPermissionSet.OWNER);
            WorldOptions options = new WorldOptions(1L, false, false);
            WorldStem stem = Util.<WorldStem>blockUntilDone(executor -> WorldLoader.load(init, context -> {
                Registry<LevelStem> noDatapackDimensions = new MappedRegistry<>(Registries.LEVEL_STEM, Lifecycle.stable()).freeze();
                WorldDimensions dimensions = context.datapackWorldRegistries().lookupOrThrow(Registries.WORLD_PRESET)
                    .getOrThrow(WorldPresets.FLAT).value().createWorldDimensions();
                WorldDimensions.Complete complete = dimensions.bake(noDatapackDimensions);
                PrimaryLevelData data = new PrimaryLevelData(settings, complete.specialWorldProperty(), complete.lifecycle());
                return new WorldLoader.DataLoadOutput<>(
                    new LevelDataAndDimensions.WorldDataAndGenSettings(data, new WorldGenSettings(options, dimensions)),
                    complete.dimensionsRegistryAccess());
            }, WorldStem::new, Util.backgroundExecutor(), executor)).get();
            return new Server(thread, access, packs, stem, output);
        }

        Server(Thread thread, LevelStorageSource.LevelStorageAccess access, PackRepository packs, WorldStem stem, Path output) {
            super(thread, access, packs, stem, Optional.of(new GameRules(WorldDataConfiguration.DEFAULT.enabledFeatures())), Proxy.NO_PROXY,
                DataFixers.getDataFixer(), new Services(null, ServicesKeySet.EMPTY, null, new Resolver(), new Resolver()),
                LoggingLevelLoadListener.forDedicatedServer(), false, new NotificationManager());
            this.output = output;
        }

        @Override protected boolean initServer() {
            this.setPlayerList(new PlayerList(this, this.registries(), this.playerDataStorage, new EmptyNotificationService()) {});
            this.loadLevel();
            return true;
        }

        @Override protected void tickServer(BooleanSupplier haveTime) {
            if (ran) return;
            ran = true;
            try {
                runWitness();
            } catch (Throwable t) {
                t.printStackTrace();
                deleteTree(tempUniverse);
                Runtime.getRuntime().halt(2);
            }
            deleteTree(tempUniverse);
            System.out.flush();
            Runtime.getRuntime().halt(0);
        }

        void cmd(String text) {
            this.getCommands().performPrefixedCommand(this.createCommandSourceStack().withSuppressedOutput(), text);
        }

        void runWitness() throws Exception {
            fEntityRandom = field(Entity.class, "random");
            fCurrentState = field(FishingHook.class, "currentState");

            ServerLevel level = this.overworld();
            GameProfile profile = new GameProfile(UUIDUtil.createOfflinePlayerUUID("Player"), "Player");
            CommonListenerCookie cookie = CommonListenerCookie.createInitial(profile, false);
            player = new ServerPlayer(this, level, profile, ClientInformation.createDefault());
            Connection connection = new Connection(PacketFlow.SERVERBOUND);
            channel = new EmbeddedChannel(connection);
            this.getPlayerList().placeNewPlayer(connection, player, cookie);
            player.setGameMode(GameType.SURVIVAL);
            player.snapTo(0.5, 64.0, 4.5, -90.0f, 0.0f);
            settle();

            cmd("gamerule spawn_mobs false");
            cmd("gamerule advance_time false");
            cmd("gamerule advance_weather false");
            cmd("gamerule mob_griefing true");
            cmd("gamerule entity_drops true");

            GameEventListener listener = new GameEventListener() {
                final PositionSource source = new BlockPositionSource(new BlockPos(3, 64, 4));
                @Override public PositionSource getListenerSource() { return source; }
                @Override public int getListenerRadius() { return 16; }
                @Override
                public boolean handleGameEvent(ServerLevel l, Holder<GameEvent> event, GameEvent.Context ctx, Vec3 pos) {
                    JsonObject ev = new JsonObject();
                    ev.addProperty("id", event.unwrapKey().map(k -> k.identifier().toString()).orElse("unknown"));
                    ev.add("pos_bits", vecBits(pos));
                    tickGameEvents.add(ev);
                    return false;
                }
            };
            level.getChunk(0, 0).getListenerRegistry(SectionPos.blockToSectionCoord(64)).register(listener);

            JsonObject root = new JsonObject();
            root.addProperty("game_version", SharedConstants.getCurrentVersion().id());
            root.addProperty("java_version", System.getProperty("java.version"));
            root.addProperty("ticks_per_case", TICKS);
            root.addProperty("hook_seed", HOOK_SEED);
            root.addProperty("level_seed", LEVEL_SEED);
            JsonArray cases = new JsonArray();
            for (String target : TARGETS) {
                cases.add(runCase(level, target));
            }
            root.add("cases", cases);
            Files.createDirectories(output.getParent());
            Gson gson = new GsonBuilder().serializeNulls().create();
            Files.writeString(output, gson.toJson(root) + "\n", StandardCharsets.UTF_8);
            System.out.println("Wrote " + cases.size() + " fishing cases to " + output);
        }

        void resetArena(ServerLevel level, String target) {
            if (player.fishing != null) {
                player.fishing.discard();
            }
            List<Entity> toDiscard = new ArrayList<>();
            for (Entity e : level.getAllEntities()) {
                if (e != player) toDiscard.add(e);
            }
            for (Entity e : toDiscard) e.discard();

            for (int x = 0; x <= 11; x++) {
                for (int z = 0; z <= 9; z++) {
                    level.setBlock(new BlockPos(x, 63, z), Blocks.STONE.defaultBlockState(), 2);
                    for (int y = 64; y <= 70; y++) {
                        level.setBlock(new BlockPos(x, y, z), Blocks.AIR.defaultBlockState(), 2);
                    }
                }
            }
            for (int y = 64; y <= 67; y++) {
                for (int z = 2; z <= 6; z++) {
                    level.setBlock(new BlockPos(10, y, z), Blocks.STONE.defaultBlockState(), 2);
                }
            }
            boolean wallTarget = target.equals("painting") || target.startsWith("item_frame") || target.startsWith("glow_item_frame");
            if (wallTarget) {
                for (int y = 64; y <= 66; y++) {
                    for (int z = 2; z <= 6; z++) {
                        level.setBlock(new BlockPos(4, y, z), Blocks.STONE.defaultBlockState(), 2);
                    }
                }
            }
            if (target.startsWith("leash_knot")) {
                level.setBlock(new BlockPos(3, 64, 4), Blocks.OAK_FENCE.defaultBlockState(), 2);
            }
            player.setGameMode(GameType.SURVIVAL);
            player.snapTo(0.5, 64.0, 4.5, -90.0f, 0.0f);
            player.setDeltaMovement(Vec3.ZERO);
            player.setItemInHand(InteractionHand.MAIN_HAND, new ItemStack(Items.FISHING_ROD));
            stepTick();
            channel.runPendingTasks();
            channel.outboundMessages().clear();
            tickGameEvents.clear();
        }

        Set<Integer> entityIds(ServerLevel level) {
            Set<Integer> ids = new HashSet<>();
            for (Entity e : level.getAllEntities()) ids.add(e.getId());
            return ids;
        }

        Entity findNewEntity(ServerLevel level, Set<Integer> before, Class<? extends Entity> cls) {
            for (Entity e : level.getAllEntities()) {
                if (!before.contains(e.getId()) && cls.isInstance(e)) return e;
            }
            return null;
        }

        double spawnY(String target) {
            if (target.equals("falling_block")) return 65.3;
            if (target.equals("end_crystal")) return 65.0;
            if (target.equals("ender_dragon")) return 65.0;
            if (target.startsWith("leash_knot")) return 64.6;
            if (target.equals("cushion")) return 64.1;
            if (target.equals("dropped_item")) return 64.15;
            return 64.5;
        }

        JsonObject runCase(ServerLevel level, String targetKind) throws Exception {
            resetArena(level, targetKind);
            level.getRandom().setSeed(LEVEL_SEED);

            Set<Integer> beforeTarget = entityIds(level);
            switch (targetKind) {
                case "item_frame" -> cmd("summon minecraft:item_frame 3.5 64.5 4.5 {block_pos:[I;3,64,4],Facing:4b,Item:{id:\"minecraft:diamond\",count:1}}");
                case "item_frame_empty" -> cmd("summon minecraft:item_frame 3.5 64.5 4.5 {block_pos:[I;3,64,4],Facing:4b}");
                case "item_frame_fixed" -> cmd("summon minecraft:item_frame 3.5 64.5 4.5 {block_pos:[I;3,64,4],Facing:4b,Fixed:1b,Item:{id:\"minecraft:diamond\",count:1}}");
                case "glow_item_frame" -> cmd("summon minecraft:glow_item_frame 3.5 64.5 4.5 {block_pos:[I;3,64,4],Facing:4b,Item:{id:\"minecraft:diamond\",count:1}}");
                case "painting" -> cmd("summon minecraft:painting 3.5 64.5 4.5 {block_pos:[I;3,64,4],facing:1b,variant:\"minecraft:kebab\"}");
                case "falling_block" -> cmd("summon minecraft:falling_block 3.5 65.0 4.5 {BlockState:\"minecraft:sand\",Time:1,Motion:[0.0d,0.0d,0.0d]}");
                case "end_crystal" -> cmd("summon minecraft:end_crystal 4.5 64.0 4.5 {}");
                case "ender_dragon" -> {
                    cmd("summon minecraft:ender_dragon 7.5 64.0 4.5 {DragonPhase:10,Silent:1b}");
                    stepTick();
                    tickGameEvents.clear();
                }
                case "armor_stand" -> cmd("summon minecraft:armor_stand 3.5 64.0 4.5 {}");
                case "armor_stand_marker" -> cmd("summon minecraft:armor_stand 3.5 64.0 4.5 {Marker:1b}");
                case "armor_stand_small" -> cmd("summon minecraft:armor_stand 3.5 64.0 4.5 {Small:1b}");
                case "armor_stand_nogravity" -> cmd("summon minecraft:armor_stand 3.5 64.0 4.5 {NoGravity:1b}");
                case "leash_knot" -> cmd("summon minecraft:leash_knot 3.5 64.375 4.5 {}");
                case "leash_knot_tied" -> {
                    cmd("summon minecraft:pig 3.5 64.0 6.5 {NoAI:1b,Silent:1b,leash:[I;3,64,4]}");
                    stepTick();
                    tickGameEvents.clear();
                }
                case "cushion" -> cmd("summon minecraft:cushion 3.5 64.0 4.5 {block_pos:[I;3,64,4],color:\"white\"}");
                case "dropped_item" -> cmd("summon minecraft:item 3.5 64.0 4.5 {Item:{id:\"minecraft:diamond\",count:1},Motion:[0.0d,0.0d,0.0d],PickupDelay:32767}");
                default -> throw new IllegalArgumentException(targetKind);
            }

            Entity target;
            if (targetKind.equals("leash_knot_tied")) {
                target = findNewEntity(level, beforeTarget, LeashFenceKnotEntity.class);
            } else if (targetKind.equals("ender_dragon")) {
                target = findNewEntity(level, beforeTarget, EnderDragon.class);
            } else {
                target = findNewEntity(level, beforeTarget, Entity.class);
            }
            if (target == null) throw new IllegalStateException("target failed to spawn: " + targetKind);

            double y0 = spawnY(targetKind);
            FishingHook hook = new FishingHook(player, level, 0, 0);
            hook.snapTo(1.5, y0, 4.5, -90.0f, 0.0f);
            hook.setDeltaMovement(new Vec3(1.0, 0.0, 0.0));
            fEntityRandom.set(hook, RandomSource.create(HOOK_SEED));
            level.addFreshEntity(hook);
            tickGameEvents.clear();

            JsonObject rec = new JsonObject();
            rec.addProperty("target", targetKind);
            rec.addProperty("spawn_y", y0);
            rec.addProperty("target_bb_height_bits", fBits(target.getBbHeight()));
            rec.add("target_pos0_bits", vecBits(target.position()));

            JsonArray ticksArr = new JsonArray();
            for (int t = 1; t <= TICKS; t++) {
                tickGameEvents.clear();
                stepTick();
                channel.runPendingTasks();
                channel.outboundMessages().clear();

                JsonObject row = new JsonObject();
                row.addProperty("tick", t);
                row.addProperty("hook_alive", !hook.isRemoved());
                row.addProperty("hook_state", String.valueOf(fCurrentState.get(hook)));
                row.add("hook_pos", vecArr(hook.position()));
                row.add("hook_pos_bits", vecBits(hook.position()));
                row.add("hook_vel_bits", vecBits(hook.getDeltaMovement()));

                Entity hooked = hook.getHookedIn();
                if (hooked == null) {
                    row.add("hooked_type", JsonNull.INSTANCE);
                    row.add("hooked_part", JsonNull.INSTANCE);
                    row.add("hooked_pos_bits", JsonNull.INSTANCE);
                    row.add("hooked_bb_height_bits", JsonNull.INSTANCE);
                } else {
                    row.addProperty("hooked_type", BuiltInRegistries.ENTITY_TYPE.getKey(hooked.getType()).toString());
                    if (hooked instanceof EnderDragonPart part) {
                        row.addProperty("hooked_part", part.name);
                    } else {
                        row.add("hooked_part", JsonNull.INSTANCE);
                    }
                    row.add("hooked_pos_bits", vecBits(hooked.position()));
                    row.addProperty("hooked_bb_height_bits", fBits(hooked.getBbHeight()));
                }

                row.addProperty("target_alive", !target.isRemoved());
                row.add("target_pos_bits", vecBits(target.position()));
                row.add("target_vel_bits", vecBits(target.getDeltaMovement()));
                JsonArray gEvents = new JsonArray();
                tickGameEvents.forEach(gEvents::add);
                row.add("game_events", gEvents);
                ticksArr.add(row);
            }
            rec.add("ticks", ticksArr);

            // Retrieve the bobber and record rod durability damage + pull velocity.
            Entity hookedBeforeRetrieve = hook.getHookedIn();
            int rodDamage = hook.isRemoved() ? -1 : hook.retrieve(player.getMainHandItem());
            rec.addProperty("retrieve_damage", rodDamage);
            rec.add("target_vel_after_retrieve_bits", vecBits(target.getDeltaMovement()));
            rec.add("hooked_vel_after_retrieve_bits",
                hookedBeforeRetrieve == null ? JsonNull.INSTANCE : vecBits(hookedBeforeRetrieve.getDeltaMovement()));
            stepTick();
            rec.add("target_pos_after_retrieve_tick_bits", vecBits(target.position()));
            rec.add("target_vel_after_retrieve_tick_bits", vecBits(target.getDeltaMovement()));
            return rec;
        }

        void stepTick() {
            drain();
            super.tickServer(() -> false);
            drain();
        }

        void drain() {
            boolean more = true;
            while (more) {
                more = false;
                this.runAllTasks();
                for (ServerLevel level : this.getAllLevels()) {
                    while (level.getChunkSource().pollTask()) {
                        more = true;
                    }
                }
            }
        }

        void settle() {
            ServerLevel level = player.level();
            for (int dz = -2; dz <= 2; dz++) {
                for (int dx = -2; dx <= 2; dx++) {
                    level.getChunkSource().addTicket(
                        new Ticket(TicketType.DRAGON, ChunkMap.FORCED_TICKET_LEVEL),
                        new ChunkPos(dx, dz));
                    level.getChunk(dx, dz);
                }
            }
            long deadline = System.nanoTime() + 120_000_000_000L;
            for (int i = 0; System.nanoTime() < deadline; i++) {
                stepTick();
                boolean ready = true;
                for (int dz = -2; dz <= 2 && ready; dz++) {
                    for (int dx = -2; dx <= 2 && ready; dx++) {
                        ready = level.areEntitiesActuallyLoadedAndTicking(new ChunkPos(dx, dz));
                    }
                }
                if (ready && i >= 5) return;
                try { Thread.sleep(5); } catch (InterruptedException e) { throw new RuntimeException(e); }
            }
            throw new IllegalStateException("chunks did not load");
        }

        @Override public LevelBasedPermissionSet getProfilePermissions(NameAndId nameAndId) { return LevelBasedPermissionSet.OWNER; }
        @Override protected SampleLogger getTickTimeLogger() { return sampleLogger; }
        @Override public boolean isTickTimeLoggingEnabled() { return false; }
        @Override public SystemReport fillServerSystemReport(SystemReport report) { return report; }
        @Override public boolean isHardcore() { return false; }
        @Override public LevelBasedPermissionSet operatorUserPermissions() { return LevelBasedPermissionSet.GAMEMASTER; }
        @Override public PermissionSet getFunctionCompilationPermissions() { return LevelBasedPermissionSet.GAMEMASTER; }
        @Override public boolean shouldRconBroadcast() { return true; }
        @Override public boolean isDedicatedServer() { return false; }
        @Override public int getRateLimitPacketsPerSecond() { return 0; }
        @Override public int getCommandSpamThresholdSeconds() { return 0; }
        @Override public int getChatSpamThresholdSeconds() { return 0; }
        @Override public boolean useNativeTransport() { return false; }
        @Override public boolean isPublished() { return false; }
        @Override public boolean shouldInformAdmins() { return true; }
        @Override public boolean isSingleplayerOwner(NameAndId nameAndId) { return nameAndId.name().equalsIgnoreCase("Player"); }
        @Override public int getMaxPlayers() { return 8; }
    }

    static void deleteTree(Path root) {
        if (root == null || !Files.exists(root)) return;
        try (var stream = Files.walk(root)) {
            stream.sorted((a, b) -> b.compareTo(a)).forEach(p -> {
                try { Files.deleteIfExists(p); } catch (Exception ignored) {}
            });
        } catch (Exception ignored) {}
    }

    static Path tempUniverse;

    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        Path universe;
        if ("-".equals(args[0])) {
            universe = Files.createTempDirectory("sunrise-fishing-targets-oracle-");
            tempUniverse = universe;
        } else {
            universe = Path.of(args[0]);
            deleteTree(universe.resolve("witness"));
            Files.createDirectories(universe);
        }
        Path output = Path.of(args[1]).toAbsolutePath();
        if (output.getParent() != null) Files.createDirectories(output.getParent());
        LevelStorageSource.LevelStorageAccess access = LevelStorageSource.createDefault(universe).createAccess("witness");
        PackRepository packs = ServerPacksSource.createPackRepository(access);
        MinecraftServer.spin(thread -> {
            try {
                return Server.create(thread, access, packs, output);
            } catch (Exception e) {
                throw new RuntimeException(e);
            }
        });
    }
}
