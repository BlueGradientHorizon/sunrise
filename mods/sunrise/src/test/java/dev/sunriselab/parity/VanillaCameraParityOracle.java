package dev.sunriselab.parity;

import com.google.gson.*;
import com.mojang.blaze3d.platform.Window;
import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.renderpearl.api.device.GpuDevice;
import java.lang.reflect.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.*;
import net.minecraft.client.*;
import net.minecraft.client.entity.ClientAvatarState;
import net.minecraft.client.gui.Gui;
import net.minecraft.client.gui.Hud;
import net.minecraft.client.gui.components.debug.DebugEntryBiome;
import net.minecraft.client.gui.components.debug.DebugEntryLight;
import net.minecraft.client.gui.components.debug.DebugEntryPosition;
import net.minecraft.client.gui.components.debug.DebugScreenDisplayer;
import net.minecraft.client.multiplayer.ClientChunkCache;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.client.multiplayer.MultiPlayerGameMode;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.client.renderer.GameRenderer;
import net.minecraft.client.renderer.SkyRenderer;
import net.minecraft.client.renderer.culling.Frustum;
import net.minecraft.client.renderer.debug.ChunkBorderRenderer;
import net.minecraft.client.renderer.debug.EntityHitboxDebugRenderer;
import net.minecraft.client.renderer.extract.LevelExtractor;
import net.minecraft.client.renderer.fog.FogData;
import net.minecraft.client.renderer.fog.environment.*;
import net.minecraft.client.renderer.state.GameRenderState;
import net.minecraft.client.renderer.state.level.CameraRenderState;
import net.minecraft.core.*;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.gizmos.CuboidGizmo;
import net.minecraft.gizmos.Gizmo;
import net.minecraft.gizmos.GizmoCollector;
import net.minecraft.gizmos.GizmoProperties;
import net.minecraft.gizmos.Gizmos;
import net.minecraft.gizmos.LineGizmo;
import net.minecraft.network.PacketProcessor;
import net.minecraft.network.protocol.game.*;
import net.minecraft.network.syncher.EntityDataAccessor;
import net.minecraft.network.syncher.SynchedEntityData;
import net.minecraft.resources.Identifier;
import net.minecraft.util.Mth;
import net.minecraft.world.TickRateManager;
import net.minecraft.world.attribute.EnvironmentAttributeProbe;
import net.minecraft.world.attribute.EnvironmentAttributes;
import net.minecraft.world.effect.MobEffectInstance;
import net.minecraft.world.effect.MobEffects;
import net.minecraft.world.entity.*;
import net.minecraft.world.entity.monster.Creeper;
import net.minecraft.world.entity.player.Abilities;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.entity.projectile.ProjectileUtil;
import net.minecraft.world.entity.vehicle.minecart.Minecart;
import net.minecraft.world.level.GameType;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.LightLayer;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.Biomes;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.border.WorldBorder;
import net.minecraft.world.level.dimension.DimensionType;
import net.minecraft.world.level.lighting.LevelLightEngine;
import net.minecraft.world.level.material.FluidState;
import net.minecraft.world.level.material.Fluids;
import net.minecraft.world.level.material.FogType;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.EntityHitResult;
import net.minecraft.world.phys.HitResult;
import net.minecraft.world.phys.Vec3;
import org.joml.Matrix4f;
import org.joml.Quaternionf;

/**
 * Real vanilla 26.3 oracle for camera parity items 1, 6, 12, and 14:
 * any tracked entity as camera, Interaction lifecycle, camera entity removal,
 * camera consumers (bob, frozen partial tick, block outline, fog, dark disc, vignette),
 * and culling frustum (max(fov, optionFov), no bob/spin).
 */
public final class VanillaCameraParityOracle {
    static Object unsafe;
    static Method allocate;

    @SuppressWarnings("unchecked")
    static <T> T make(Class<T> type) throws Exception {
        return (T) allocate.invoke(unsafe, type);
    }

    static Field field(Class<?> owner, String name) throws Exception {
        for (Class<?> c = owner; c != null; c = c.getSuperclass()) {
            try {
                Field f = c.getDeclaredField(name);
                f.setAccessible(true);
                return f;
            } catch (NoSuchFieldException ignored) {
            }
        }
        throw new NoSuchFieldException(owner.getName() + "." + name);
    }

    static Method method(Class<?> owner, String name, Class<?>... params) throws Exception {
        for (Class<?> c = owner; c != null; c = c.getSuperclass()) {
            try {
                Method m = c.getDeclaredMethod(name, params);
                m.setAccessible(true);
                return m;
            } catch (NoSuchMethodException ignored) {
            }
        }
        throw new NoSuchMethodException(owner.getName() + "." + name);
    }

    static String fb(float v) {
        return String.format("%08x", Float.isNaN(v) ? 0x7fc00000 : Float.floatToRawIntBits(v));
    }

    static String db(double v) {
        return String.format("%016x", Double.isNaN(v) ? 0x7ff8000000000000L : Double.doubleToRawLongBits(v));
    }

    static JsonArray vec3Bits(Vec3 v) {
        var a = new JsonArray();
        a.add(db(v.x));
        a.add(db(v.y));
        a.add(db(v.z));
        return a;
    }

    static JsonArray vec3fBits(float x, float y, float z) {
        var a = new JsonArray();
        a.add(fb(x));
        a.add(fb(y));
        a.add(fb(z));
        return a;
    }

    static JsonArray quatBits(Quaternionf q) {
        var a = new JsonArray();
        a.add(fb(q.x));
        a.add(fb(q.y));
        a.add(fb(q.z));
        a.add(fb(q.w));
        return a;
    }

    static JsonArray mat4Bits(Matrix4f m) {
        float[] cols = new float[16];
        m.get(cols);
        var a = new JsonArray();
        for (float v : cols) a.add(fb(v));
        return a;
    }

    static JsonArray aabbBits(AABB b) {
        var a = new JsonArray();
        a.add(db(b.minX));
        a.add(db(b.minY));
        a.add(db(b.minZ));
        a.add(db(b.maxX));
        a.add(db(b.maxY));
        a.add(db(b.maxZ));
        return a;
    }

    static final class FixedDelta implements DeltaTracker {
        final float realPartial;
        final float gamePartial;
        final float deltaTicks;

        FixedDelta(float gamePartial, float realPartial, float deltaTicks) {
            this.gamePartial = gamePartial;
            this.realPartial = realPartial;
            this.deltaTicks = deltaTicks;
        }

        @Override public float getGameTimeDeltaTicks() { return deltaTicks; }
        @Override public float getGameTimeDeltaPartialTick(boolean ignoreFreeze) { return ignoreFreeze ? realPartial : gamePartial; }
        @Override public float getRealtimeDeltaTicks() { return deltaTicks; }
    }

    static final class StubProbe extends EnvironmentAttributeProbe {
        float waterFogStart = -8.0F;
        float waterFogEnd = 96.0F;

        @Override public void tick(Level level, Vec3 position) { }

        @SuppressWarnings("unchecked")
        @Override
        public <V> V getValue(net.minecraft.world.attribute.EnvironmentAttribute<V> attr, float partial) {
            if (attr == EnvironmentAttributes.WATER_FOG_START_DISTANCE) return (V) Float.valueOf(waterFogStart);
            if (attr == EnvironmentAttributes.WATER_FOG_END_DISTANCE) return (V) Float.valueOf(waterFogEnd);
            return super.getValue(attr, partial);
        }
    }

    static final class StubLightEngine extends LevelLightEngine {
        StubClientLevel owner;

        StubLightEngine() {
            super(null, false, false);
        }

        @Override
        public int getRawBrightness(BlockPos pos, int darken) {
            return owner.brightnessMap.getOrDefault(pos, owner.defaultBrightness);
        }
    }

    static final class StubChunkCache extends ClientChunkCache {
        LevelLightEngine light;

        StubChunkCache() {
            super(null, 0);
        }

        @Override
        public LevelLightEngine getLightEngine() {
            return light;
        }
    }

    static final class StubLevelExtractor extends LevelExtractor {
        StubLevelExtractor() {
            super(null, null, null);
        }

        @Override
        public boolean isEntityVisible(Entity e, Frustum f, double x, double y, double z, float pt, long fade) {
            return true;
        }
    }

    static final class StubClientLevel extends ClientLevel {
        Map<Integer, Entity> entities = new HashMap<>();
        TickRateManager tickRateManager = new TickRateManager();
        WorldBorder worldBorder = new WorldBorder();
        Map<BlockPos, Integer> brightnessMap = new HashMap<>();
        int defaultBrightness = 15;
        DimensionType dimType;
        boolean flat = false;
        StubChunkCache chunkCache;
        Map<Long, Holder<Biome>> chunkBiomes = new HashMap<>();
        Holder<Biome> defaultBiome;

        StubClientLevel() {
            super(null, null, null, null, 0, 0, null, false, 0L, 0);
        }

        @Override public boolean isClientSide() { return true; }
        @Override public net.minecraft.world.flag.FeatureFlagSet enabledFeatures() { return net.minecraft.world.flag.FeatureFlags.VANILLA_SET; }
        @Override public RegistryAccess registryAccess() { return ParityData.access; }
        @Override public DimensionType dimensionType() { return dimType; }
        @Override public TickRateManager tickRateManager() { return tickRateManager; }
        @Override public WorldBorder getWorldBorder() { return worldBorder; }
        @Override public Entity getEntity(int id) { return entities.get(id); }
        @Override public Iterable<Entity> entitiesForRendering() { return entities.values(); }
        @Override public boolean isTickingEntity(Entity e) { return entities.containsKey(e.getId()); }
        @Override public BlockState getBlockState(BlockPos p) { return Blocks.AIR.defaultBlockState(); }
        @Override public FluidState getFluidState(BlockPos p) { return Fluids.EMPTY.defaultFluidState(); }
        @Override public int getMaxLocalRawBrightness(BlockPos pos) { return brightnessMap.getOrDefault(pos, defaultBrightness); }
        @Override public int getBrightness(LightLayer layer, BlockPos pos) {
            return layer == LightLayer.SKY ? brightnessMap.getOrDefault(pos, defaultBrightness) : 0;
        }
        @Override public ClientChunkCache getChunkSource() { return chunkCache; }
        @Override public Holder<Biome> getBiome(BlockPos pos) {
            long key = ((long) (pos.getX() >> 4) << 32) ^ ((pos.getZ() >> 4) & 0xffffffffL);
            return chunkBiomes.getOrDefault(key, defaultBiome);
        }
        @Override
        public List<Entity> getEntities(Entity except, AABB box, java.util.function.Predicate<? super Entity> pred) {
            List<Entity> hits = new ArrayList<>();
            for (Entity e : entities.values()) {
                if (e != except && e.getBoundingBox().intersects(box) && pred.test(e)) {
                    hits.add(e);
                }
            }
            return hits;
        }

        @Override
        public void removeEntity(int id, Entity.RemovalReason reason) {
            Entity e = entities.remove(id);
            if (e != null) {
                e.setRemoved(reason);
                e.onClientRemoval();
            }
        }
    }

    static final class StubCamera extends Camera {
        FogType fluid = FogType.NONE;
        @Override public FogType getFluidInCamera() { return fluid; }
    }

    static final class StubLocalPlayer extends LocalPlayer {
        boolean spectatorFlag;
        boolean underWaterFlag;
        float waterVision = 1.0F;
        ClientAvatarState avatar = new ClientAvatarState();

        StubLocalPlayer() {
            super(null, null, null, null, null, net.minecraft.world.entity.player.Input.EMPTY, false, null, null);
        }

        @Override public boolean isSpectator() { return spectatorFlag; }
        @Override public boolean isUnderWater() { return underWaterFlag; }
        @Override public float getWaterVision() { return waterVision; }
        @Override public ClientAvatarState avatarState() { return avatar; }
        @Override public boolean isSleeping() { return false; }
        @Override public boolean isDeadOrDying() { return false; }
        @Override public boolean isDescending() { return false; }
        @Override public boolean isInvisible() { return false; }
        @Override public net.minecraft.world.item.ItemStack getMainHandItem() { return net.minecraft.world.item.ItemStack.EMPTY; }
    }

    static final class StubGameMode extends MultiPlayerGameMode {
        GameType mode = GameType.CREATIVE;
        StubGameMode() { super(null, null); }
        @Override public GameType getPlayerMode() { return mode; }
    }

    static Minecraft minecraft;
    static Options options;
    static StubClientLevel level;
    static DimensionType overworldDimType;

    static void installFakeGpuDevice() throws Exception {
        var deviceInfo = new com.mojang.renderpearl.api.device.DeviceInfo(
            "oracle", "oracle", "oracle", true, "oracle", 1.0F, null, null, Set.of(), null, null
        );
        Object device = Proxy.newProxyInstance(
            GpuDevice.class.getClassLoader(),
            new Class<?>[] { GpuDevice.class },
            (proxy, m, a) -> {
                if (m.getName().equals("getDeviceInfo")) return deviceInfo;
                return null;
            }
        );
        field(RenderSystem.class, "DEVICE").set(null, device);
    }

    static StubClientLevel newLevel() throws Exception {
        var lvl = make(StubClientLevel.class);
        lvl.entities = new HashMap<>();
        lvl.tickRateManager = new TickRateManager();
        lvl.worldBorder = new WorldBorder();
        lvl.brightnessMap = new HashMap<>();
        lvl.defaultBrightness = 15;
        lvl.dimType = overworldDimType;
        lvl.chunkBiomes = new HashMap<>();
        lvl.defaultBiome = ParityData.access.lookupOrThrow(Registries.BIOME).getOrThrow(Biomes.PLAINS);
        var light = make(StubLightEngine.class);
        light.owner = lvl;
        var cache = make(StubChunkCache.class);
        cache.light = light;
        lvl.chunkCache = cache;
        var levelData = new ClientLevel.ClientLevelData(net.minecraft.world.Difficulty.NORMAL, false, false);
        field(ClientLevel.class, "clientLevelData").set(lvl, levelData);
        field(Level.class, "levelData").set(lvl, levelData);
        field(Level.class, "dimension").set(lvl, Level.OVERWORLD);
        field(Level.class, "dimensionTypeRegistration").set(lvl, Holder.direct(overworldDimType));
        field(Level.class, "registryAccess").set(lvl, ParityData.access);
        field(Level.class, "isClientSide").setBoolean(lvl, true);
        field(Level.class, "random").set(lvl, net.minecraft.util.RandomSource.create(0L));
        field(ClientLevel.class, "minecraft").set(lvl, minecraft);
        return lvl;
    }

    static StubCamera newCamera(StubClientLevel lvl) throws Exception {
        var cam = new StubCamera();
        field(Camera.class, "attributeProbe").set(cam, new StubProbe());
        field(Camera.class, "level").set(cam, lvl);
        return cam;
    }

    static StubLocalPlayer newPlayer(StubClientLevel lvl, int id, double x, double y, double z) throws Exception {
        var p = make(StubLocalPlayer.class);
        p.avatar = new ClientAvatarState();
        p.waterVision = 1.0F;
        p.setId(id);
        field(Entity.class, "level").set(p, lvl);
        field(LocalPlayer.class, "minecraft").set(p, minecraft);
        field(Entity.class, "position").set(p, new Vec3(x, y, z));
        field(Entity.class, "blockPosition").set(p, BlockPos.containing(x, y, z));
        p.xo = x; p.yo = y; p.zo = z;
        var dims = EntityDimensions.scalable(0.6F, 1.8F).withEyeHeight(1.62F);
        field(Entity.class, "eyeHeight").setFloat(p, 1.62F);
        field(Entity.class, "dimensions").set(p, dims);
        p.setBoundingBox(dims.makeBoundingBox(x, y, z));
        field(Entity.class, "passengers").set(p, com.google.common.collect.ImmutableList.of());
        field(LivingEntity.class, "activeEffects").set(p, new HashMap<Holder<net.minecraft.world.effect.MobEffect>, MobEffectInstance>());
        var ab = new Abilities();
        ab.mayBuild = true;
        field(Player.class, "abilities").set(p, ab);
        lvl.entities.put(id, p);
        return p;
    }

    static Entity spawnViaPacket(StubClientLevel lvl, String typeId, int id, double x, double y, double z, float yRot, float xRot, float yHeadRot) throws Exception {
        EntityType<?> type = BuiltInRegistries.ENTITY_TYPE.getValue(Identifier.parse(typeId));
        Entity e = type.create(lvl, EntitySpawnReason.LOAD);
        if (e == null) throw new IllegalStateException("Could not create entity " + typeId);
        var pkt = new ClientboundAddEntityPacket(id, new UUID(0L, id), x, y, z, xRot, yRot, type, 0, Vec3.ZERO, yHeadRot);
        e.recreateFromPacket(pkt);
        lvl.entities.put(id, e);
        return e;
    }

    @SuppressWarnings("unchecked")
    static EntityDataAccessor<Float> interactionWidthAccessor() throws Exception {
        return (EntityDataAccessor<Float>) field(Interaction.class, "DATA_WIDTH_ID").get(null);
    }

    @SuppressWarnings("unchecked")
    static EntityDataAccessor<Float> interactionHeightAccessor() throws Exception {
        return (EntityDataAccessor<Float>) field(Interaction.class, "DATA_HEIGHT_ID").get(null);
    }

    @SuppressWarnings("unchecked")
    static EntityDataAccessor<Pose> entityPoseAccessor() throws Exception {
        return (EntityDataAccessor<Pose>) field(Entity.class, "DATA_POSE").get(null);
    }

    static JsonObject recordEntityCameras() throws Exception {
        var out = new JsonObject();
        var align = method(Camera.class, "alignWithEntity", float.class);
        String[] types = {
            "minecraft:interaction",
            "minecraft:text_display",
            "minecraft:block_display",
            "minecraft:item_display",
            "minecraft:marker",
            "minecraft:minecart",
            "minecraft:chest_minecart",
            "minecraft:end_crystal",
            "minecraft:area_effect_cloud",
            "minecraft:arrow",
            "minecraft:experience_orb",
            "minecraft:item",
            "minecraft:falling_block",
            "minecraft:snowball",
            "minecraft:firework_rocket",
            "minecraft:lightning_bolt",
            "minecraft:creeper",
        };
        var records = new JsonArray();
        int nextId = 100;
        for (boolean syncedInteraction : new boolean[] { false, true }) {
            for (String typeId : types) {
                if (syncedInteraction && !typeId.equals("minecraft:interaction")) continue;
                var lvl = newLevel();
                var player = newPlayer(lvl, 1, 0.5, 64.0, 0.5);
                minecraft.player = player;
                minecraft.level = lvl;
                options.setCameraType(CameraType.FIRST_PERSON);
                options.fov().set(70);

                int id = nextId++;
                Entity e = spawnViaPacket(lvl, typeId, id, 12.25, 70.5, -4.75, 45.0F, 15.0F, 45.0F);
                if (syncedInteraction) {
                    e.getEntityData().assignValues(List.of(
                        SynchedEntityData.DataValue.create(interactionWidthAccessor(), 0.1F),
                        SynchedEntityData.DataValue.create(interactionHeightAccessor(), 1.9059F)
                    ));
                }

                var cam = newCamera(lvl);
                var renderer = make(GameRenderer.class);
                field(GameRenderer.class, "mainCamera").set(renderer, cam);
                field(Minecraft.class, "gameRenderer").set(minecraft, renderer);
                cam.setEntity(e);
                cam.tick();
                align.invoke(cam, 1.0F);

                var state = new CameraRenderState();
                cam.extractRenderState(state, new FixedDelta(1.0F, 1.0F, 1.0F));

                var row = new JsonObject();
                row.addProperty("label", syncedInteraction ? "minecraft:interaction_synced" : typeId);
                row.addProperty("type", typeId);
                row.addProperty("synced_genpvp", syncedInteraction);
                row.addProperty("eye_height_bits", fb(e.getEyeHeight()));
                row.addProperty("entity_yRot_bits", fb(e.getYRot()));
                row.addProperty("entity_xRot_bits", fb(e.getXRot()));
                row.add("camera_pos_after_1_tick_bits", vec3Bits(cam.position()));
                row.addProperty("camera_yRot_bits", fb(cam.yRot()));
                row.addProperty("camera_xRot_bits", fb(cam.xRot()));
                row.add("camera_forwards_bits", vec3fBits(cam.forwardVector().x(), cam.forwardVector().y(), cam.forwardVector().z()));
                row.add("camera_rotation_bits", quatBits(cam.rotation()));
                row.addProperty("is_living", state.entityRenderState.isLiving);
                row.addProperty("is_player", state.entityRenderState.isPlayer);
                row.addProperty("is_sleeping", state.entityRenderState.isSleeping);
                row.addProperty("does_mob_effect_block_sky", state.entityRenderState.doesMobEffectBlockSky);

                // Also record settled camera position (after 40 ticks so eyeHeight eases fully).
                for (int t = 0; t < 40; t++) cam.tick();
                align.invoke(cam, 1.0F);
                row.add("camera_pos_settled_bits", vec3Bits(cam.position()));
                records.add(row);
            }
        }
        out.add("records", records);
        return out;
    }

    static JsonObject snapshotInteraction(Interaction e, Entity rider) {
        var obj = new JsonObject();
        obj.add("pos_bits", vec3Bits(e.position()));
        obj.addProperty("yRot_bits", fb(e.getYRot()));
        obj.addProperty("xRot_bits", fb(e.getXRot()));
        obj.addProperty("yRotO_bits", fb(e.yRotO));
        obj.addProperty("xRotO_bits", fb(e.xRotO));
        obj.addProperty("eye_height_bits", fb(e.getEyeHeight()));
        obj.add("aabb_bits", aabbBits(e.getBoundingBox()));
        // Read real Entity.dimensions-based seat height (what positionRider uses: this.dimensions!).
        try {
            var getAttach = method(Entity.class, "getPassengerAttachmentPoint", Entity.class, EntityDimensions.class, float.class);
            Vec3 seat = (Vec3) getAttach.invoke(e, rider, e.getDimensions(Pose.STANDING), 1.0F);
            EntityDimensions dims = (EntityDimensions) field(Entity.class, "dimensions").get(e);
            Vec3 realSeat = (Vec3) getAttach.invoke(e, rider, dims, 1.0F);
            obj.addProperty("seat_height_bits", db(realSeat.y));
            obj.addProperty("dims_width_bits", fb(dims.width()));
            obj.addProperty("dims_height_bits", fb(dims.height()));
            obj.addProperty("dims_eye_height_bits", fb(dims.eyeHeight()));
            obj.addProperty("get_dims_seat_y_bits", db(seat.y));
        } catch (Exception ex) {
            throw new RuntimeException(ex);
        }
        return obj;
    }

    static JsonObject recordInteractionLifecycle() throws Exception {
        var out = new JsonObject();
        var align = method(Camera.class, "alignWithEntity", float.class);
        var lvl = newLevel();
        var rider = newPlayer(lvl, 1, 0.0, 64.0, 0.0);
        minecraft.player = rider;
        minecraft.level = lvl;

        // 1. Unsynced spawn
        Interaction i1 = (Interaction) spawnViaPacket(lvl, "minecraft:interaction", 200, 0.5, 300.0, 0.5, 90.0F, 30.0F, 90.0F);
        out.add("unsynced_spawn", snapshotInteraction(i1, rider));

        // 2. Sync explicit default (1.0F, 1.0F) on unsynced
        i1.getEntityData().assignValues(List.of(
            SynchedEntityData.DataValue.create(interactionWidthAccessor(), 1.0F),
            SynchedEntityData.DataValue.create(interactionHeightAccessor(), 1.0F)
        ));
        out.add("sync_default_1x1", snapshotInteraction(i1, rider));

        // 3. Sync DATA_POSE alone on a fresh unsynced Interaction
        Interaction iPose = (Interaction) spawnViaPacket(lvl, "minecraft:interaction", 201, 0.5, 300.0, 0.5, 0.0F, 0.0F, 0.0F);
        iPose.getEntityData().assignValues(List.of(
            SynchedEntityData.DataValue.create(entityPoseAccessor(), Pose.STANDING)
        ));
        out.add("sync_pose_only", snapshotInteraction(iPose, rider));

        // 4. Sync genpvp dimensions (0.1F, 1.9059F)
        Interaction iGenpvp = (Interaction) spawnViaPacket(lvl, "minecraft:interaction", 202, 0.5, 300.0, 0.5, 0.0F, 0.0F, 0.0F);
        iGenpvp.getEntityData().assignValues(List.of(
            SynchedEntityData.DataValue.create(interactionWidthAccessor(), 0.1F),
            SynchedEntityData.DataValue.create(interactionHeightAccessor(), 1.9059F)
        ));
        out.add("sync_genpvp", snapshotInteraction(iGenpvp, rider));

        // 5. Negative height and negative width+height
        Interaction iNegH = (Interaction) spawnViaPacket(lvl, "minecraft:interaction", 203, 10.0, 64.0, -3.0, 0.0F, 0.0F, 0.0F);
        iNegH.getEntityData().assignValues(List.of(
            SynchedEntityData.DataValue.create(interactionWidthAccessor(), 0.5F),
            SynchedEntityData.DataValue.create(interactionHeightAccessor(), -1.5F)
        ));
        out.add("sync_negative_height", snapshotInteraction(iNegH, rider));

        Interaction iNegWH = (Interaction) spawnViaPacket(lvl, "minecraft:interaction", 204, 10.0, 64.0, -3.0, 0.0F, 0.0F, 0.0F);
        iNegWH.getEntityData().assignValues(List.of(
            SynchedEntityData.DataValue.create(interactionWidthAccessor(), -0.4F),
            SynchedEntityData.DataValue.create(interactionHeightAccessor(), -2.0F)
        ));
        out.add("sync_negative_width_and_height", snapshotInteraction(iNegWH, rider));

        // 6. Rotation clamping on snapTo / setXRot / setYRot
        Interaction iClamp = (Interaction) spawnViaPacket(lvl, "minecraft:interaction", 205, 0.0, 64.0, 0.0, 0.0F, 0.0F, 0.0F);
        iClamp.snapTo(0.0, 64.0, 0.0, 405.0F, 120.0F);
        out.add("clamp_pos_pitch", snapshotInteraction(iClamp, rider));
        iClamp.snapTo(0.0, 64.0, 0.0, -405.0F, -135.0F);
        out.add("clamp_neg_pitch", snapshotInteraction(iClamp, rider));

        // 7. Movement, relative teleport, far teleport snap, and camera interpolation
        var moveLvl = newLevel();
        var movePlayer = newPlayer(moveLvl, 1, 0.0, 64.0, 0.0);
        minecraft.player = movePlayer;
        minecraft.level = moveLvl;
        var listener = make(ClientPacketListener.class);
        field(ClientPacketListener.class, "minecraft").set(listener, minecraft);
        field(ClientPacketListener.class, "level").set(listener, moveLvl);
        field(ClientPacketListener.class, "removedPlayerVehicleId").set(listener, OptionalInt.empty());

        var cam = newCamera(moveLvl);
        var renderer = make(GameRenderer.class);
        field(GameRenderer.class, "mainCamera").set(renderer, cam);
        field(Minecraft.class, "gameRenderer").set(minecraft, renderer);
        options.setCameraType(CameraType.FIRST_PERSON);

        Interaction mover = (Interaction) spawnViaPacket(moveLvl, "minecraft:interaction", 210, 10.0, 64.0, -5.0, 0.0F, 0.0F, 0.0F);
        mover.getEntityData().assignValues(List.of(
            SynchedEntityData.DataValue.create(interactionWidthAccessor(), 0.1F),
            SynchedEntityData.DataValue.create(interactionHeightAccessor(), 1.9059F)
        ));
        cam.setEntity(mover);
        for (int t = 0; t < 40; t++) cam.tick();

        // Tick 1: commonTick snapshots old pose, then MoveEntity PosRot (+1.0, +0.5, -0.25), yaw 45, pitch 15.
        mover.commonTick();
        mover.tick();
        var posRot = new ClientboundMoveEntityPacket.PosRot(
            210,
            new VecDelta.Linear((short) 4096, (short) 2048, (short) -1024),
            Mth.packDegrees(45.0F),
            Mth.packDegrees(15.0F),
            false
        );
        listener.handleMoveEntity(posRot);

        var moveFrames = new JsonArray();
        for (float pt : new float[] { 0.0F, 0.25F, 0.5F, 1.0F }) {
            align.invoke(cam, pt);
            var f = new JsonObject();
            f.addProperty("partial_bits", fb(pt));
            f.add("camera_pos_bits", vec3Bits(cam.position()));
            f.addProperty("camera_yRot_bits", fb(cam.yRot()));
            f.addProperty("camera_xRot_bits", fb(cam.xRot()));
            moveFrames.add(f);
        }
        out.add("after_move_pos_rot_frames", moveFrames);

        // Tick 2: commonTick snapshots old pose (at 11, 64.5, -5.25; yaw 45, pitch 15), then TeleportEntity with relative Y_ROT & X_ROT (+30, -10).
        mover.commonTick();
        mover.tick();
        var relTp = ClientboundTeleportEntityPacket.teleport(
            210,
            new PositionMoveRotation(new Vec3(12.0, 65.0, -4.0), Vec3.ZERO, 30.0F, -10.0F),
            EnumSet.of(Relative.Y_ROT, Relative.X_ROT),
            false
        );
        listener.handleTeleportEntity(relTp);
        var relTpFrames = new JsonArray();
        for (float pt : new float[] { 0.0F, 0.5F, 1.0F }) {
            align.invoke(cam, pt);
            var f = new JsonObject();
            f.addProperty("partial_bits", fb(pt));
            f.add("camera_pos_bits", vec3Bits(cam.position()));
            f.addProperty("camera_yRot_bits", fb(cam.yRot()));
            f.addProperty("camera_xRot_bits", fb(cam.xRot()));
            relTpFrames.add(f);
        }
        out.add("after_relative_teleport_frames", relTpFrames);

        // Tick 3: commonTick, then far TeleportEntity (> 64 blocks away, distanceToSqr > 4096) -> snaps old pos/rot.
        mover.commonTick();
        mover.tick();
        var farTp = ClientboundTeleportEntityPacket.teleport(
            210,
            new PositionMoveRotation(new Vec3(100.0, 65.0, -4.0), Vec3.ZERO, 180.0F, -20.0F),
            EnumSet.noneOf(Relative.class),
            false
        );
        listener.handleTeleportEntity(farTp);
        var farTpFrames = new JsonArray();
        for (float pt : new float[] { 0.0F, 0.5F, 1.0F }) {
            align.invoke(cam, pt);
            var f = new JsonObject();
            f.addProperty("partial_bits", fb(pt));
            f.add("camera_pos_bits", vec3Bits(cam.position()));
            f.addProperty("camera_yRot_bits", fb(cam.yRot()));
            f.addProperty("camera_xRot_bits", fb(cam.xRot()));
            farTpFrames.add(f);
        }
        out.add("after_far_teleport_frames", farTpFrames);
        return out;
    }

    static JsonObject recordCameraRemoval() throws Exception {
        var out = new JsonObject();
        var align = method(Camera.class, "alignWithEntity", float.class);
        var isControlled = method(LocalPlayer.class, "isControlledCamera");
        for (String kind : new String[] { "minecraft:interaction", "minecraft:creeper" }) {
            var lvl = newLevel();
            var player = newPlayer(lvl, 7, 0.5, 64.0, 0.5);
            minecraft.player = player;
            minecraft.level = lvl;

            var cam = newCamera(lvl);
            cam.setEntity(player);
            var renderer = make(GameRenderer.class);
            field(GameRenderer.class, "mainCamera").set(renderer, cam);
            field(GameRenderer.class, "minecraft").set(renderer, minecraft);
            field(Minecraft.class, "gameRenderer").set(minecraft, renderer);
            options.setCameraType(CameraType.FIRST_PERSON);

            var listener = make(ClientPacketListener.class);
            field(ClientPacketListener.class, "minecraft").set(listener, minecraft);
            field(ClientPacketListener.class, "level").set(listener, lvl);
            field(ClientPacketListener.class, "removedPlayerVehicleId").set(listener, OptionalInt.empty());
            field(ClientPacketListener.class, "debugSubscriber").set(
                listener, new net.minecraft.client.multiplayer.ClientDebugSubscriber(listener, null)
            );

            Entity target = spawnViaPacket(lvl, kind, 31, 10.5, 300.0, 12.5, 45.0F, 10.0F, 45.0F);
            if (target instanceof Interaction) {
                target.getEntityData().assignValues(List.of(
                    SynchedEntityData.DataValue.create(interactionWidthAccessor(), 0.1F),
                    SynchedEntityData.DataValue.create(interactionHeightAccessor(), 1.9059F)
                ));
            }

            var setTarget = make(ClientboundSetCameraPacket.class);
            field(ClientboundSetCameraPacket.class, "cameraId").setInt(setTarget, 31);
            listener.handleSetCamera(setTarget);
            for (int t = 0; t < 40; t++) cam.tick();
            align.invoke(cam, 1.0F);

            var before = new JsonObject();
            before.addProperty("camera_id", cam.entity().getId());
            before.addProperty("is_removed", cam.entity().isRemoved());
            before.addProperty("is_controlled_camera", (boolean) isControlled.invoke(player));
            before.addProperty("post_effect", renderer.spectatedEntityPostEffect() == null ? null : renderer.spectatedEntityPostEffect().toString());
            before.add("camera_pos_bits", vec3Bits(cam.position()));

            var remove = new ClientboundRemoveEntitiesPacket(31);
            listener.handleRemoveEntities(remove);
            cam.tick();
            align.invoke(cam, 1.0F);

            var afterRemove = new JsonObject();
            afterRemove.addProperty("camera_id", cam.entity().getId());
            afterRemove.addProperty("is_removed", cam.entity().isRemoved());
            afterRemove.addProperty("in_level", lvl.getEntity(31) != null);
            afterRemove.addProperty("is_controlled_camera", (boolean) isControlled.invoke(player));
            afterRemove.addProperty("post_effect", renderer.spectatedEntityPostEffect() == null ? null : renderer.spectatedEntityPostEffect().toString());
            afterRemove.add("camera_pos_bits", vec3Bits(cam.position()));
            afterRemove.addProperty("camera_yRot_bits", fb(cam.yRot()));
            afterRemove.addProperty("camera_xRot_bits", fb(cam.xRot()));

            var setPlayer = make(ClientboundSetCameraPacket.class);
            field(ClientboundSetCameraPacket.class, "cameraId").setInt(setPlayer, 7);
            listener.handleSetCamera(setPlayer);

            var afterRestore = new JsonObject();
            afterRestore.addProperty("camera_id", cam.entity().getId());
            afterRestore.addProperty("is_removed", cam.entity().isRemoved());
            afterRestore.addProperty("is_controlled_camera", (boolean) isControlled.invoke(player));
            afterRestore.addProperty("post_effect", renderer.spectatedEntityPostEffect() == null ? null : renderer.spectatedEntityPostEffect().toString());

            var caseObj = new JsonObject();
            caseObj.add("after_set_camera", before);
            caseObj.add("after_remove_entities", afterRemove);
            caseObj.add("after_set_camera_player", afterRestore);
            out.add(kind, caseObj);
        }
        return out;
    }

    static JsonObject recordCameraConsumers() throws Exception {
        var out = new JsonObject();
        var bobHurt = method(GameRenderer.class, "bobHurt", CameraRenderState.class, PoseStack.class);
        var bobView = method(GameRenderer.class, "bobView", CameraRenderState.class, PoseStack.class);
        var shouldOutline = method(GameRenderer.class, "shouldRenderBlockOutline");
        var shouldDarkDisc = method(SkyRenderer.class, "shouldRenderDarkDisc", float.class, ClientLevel.class);
        var updateVignette = method(Hud.class, "updateVignetteBrightness", Entity.class);

        // --- 1. Bob (bobHurt + bobView) ---
        {
            var lvl = newLevel();
            var player = newPlayer(lvl, 1, 0.5, 64.0, 0.5);
            player.hurtTime = 6;
            player.hurtDuration = 10;
            field(Player.class, "hurtDir").setFloat(player, 45.0F);
            player.avatar.addWalkDistance(1.75F);
            player.avatar.tick(player.position(), Vec3.ZERO);
            field(ClientAvatarState.class, "bob").setFloat(player.avatar, 0.08F);
            field(ClientAvatarState.class, "bobO").setFloat(player.avatar, 0.04F);

            var interaction = (Interaction) spawnViaPacket(lvl, "minecraft:interaction", 2, 0.5, 300.0, 0.5, 0.0F, 0.0F, 0.0F);
            var creeper = (Creeper) spawnViaPacket(lvl, "minecraft:creeper", 3, 5.0, 64.0, 5.0, 0.0F, 0.0F, 0.0F);
            creeper.hurtTime = 6;
            creeper.hurtDuration = 10;

            minecraft.player = player;
            minecraft.level = lvl;
            var cam = newCamera(lvl);
            var renderer = make(GameRenderer.class);
            field(GameRenderer.class, "mainCamera").set(renderer, cam);
            field(GameRenderer.class, "minecraft").set(renderer, minecraft);
            var grs = new GameRenderState();
            grs.optionsRenderState.damageTiltStrength = 1.0;
            grs.optionsRenderState.bobView = true;
            field(GameRenderer.class, "gameRenderState").set(renderer, grs);
            field(Minecraft.class, "gameRenderer").set(minecraft, renderer);

            var bobObj = new JsonObject();
            var delta = new FixedDelta(0.5F, 0.5F, 1.0F);
            for (var entry : List.of(Map.entry("local_player", (Entity) player), Map.entry("interaction", (Entity) interaction), Map.entry("creeper", (Entity) creeper))) {
                cam.setEntity(entry.getValue());
                var cs = new CameraRenderState();
                cam.extractRenderState(cs, delta);
                var stack = new PoseStack();
                bobHurt.invoke(renderer, cs, stack);
                bobView.invoke(renderer, cs, stack);
                var row = new JsonObject();
                row.addProperty("is_living", cs.entityRenderState.isLiving);
                row.addProperty("is_player", cs.entityRenderState.isPlayer);
                row.add("matrix_bits", mat4Bits(stack.last().pose()));
                bobObj.add(entry.getKey(), row);
            }
            out.add("bob", bobObj);
        }

        // --- 2. Frozen-level camera partial tick ---
        {
            var lvl = newLevel();
            lvl.tickRateManager.setFrozen(true);
            lvl.tickRateManager.tick();
            var player = newPlayer(lvl, 1, 0.5, 64.0, 0.5);
            var interaction = (Interaction) spawnViaPacket(lvl, "minecraft:interaction", 2, 0.5, 300.0, 0.5, 0.0F, 0.0F, 0.0F);
            var creeper = (Creeper) spawnViaPacket(lvl, "minecraft:creeper", 3, 5.0, 64.0, 5.0, 0.0F, 0.0F, 0.0F);
            var cart = (Minecart) spawnViaPacket(lvl, "minecraft:minecart", 4, 5.0, 64.0, 5.0, 0.0F, 0.0F, 0.0F);
            field(Entity.class, "passengers").set(cart, com.google.common.collect.ImmutableList.of(player));

            var cam = newCamera(lvl);
            var delta = new FixedDelta(1.0F, 0.375F, 0.0F);
            var frozenObj = new JsonObject();
            frozenObj.addProperty("real_partial_bits", fb(0.375F));
            for (var entry : List.of(
                Map.entry("local_player", (Entity) player),
                Map.entry("interaction", (Entity) interaction),
                Map.entry("creeper", (Entity) creeper),
                Map.entry("minecart_with_player", (Entity) cart)
            )) {
                cam.setEntity(entry.getValue());
                frozenObj.addProperty(entry.getKey() + "_partial_bits", fb(cam.getCameraEntityPartialTicks(delta)));
            }
            out.add("frozen_partial", frozenObj);
        }

        // --- 3. Block outline gate ---
        {
            var lvl = newLevel();
            var player = newPlayer(lvl, 1, 0.5, 64.0, 0.5);
            var interaction = (Interaction) spawnViaPacket(lvl, "minecraft:interaction", 2, 0.5, 300.0, 0.5, 0.0F, 0.0F, 0.0F);
            var creeper = (Creeper) spawnViaPacket(lvl, "minecraft:creeper", 3, 5.0, 64.0, 5.0, 0.0F, 0.0F, 0.0F);
            minecraft.player = player;
            minecraft.level = lvl;

            var gm = make(StubGameMode.class);
            gm.mode = GameType.CREATIVE;
            field(Minecraft.class, "gameMode").set(minecraft, gm);

            var hud = make(Hud.class);
            field(Hud.class, "minecraft").set(hud, minecraft);
            var gui = make(Gui.class);
            field(Gui.class, "hud").set(gui, hud);
            field(Minecraft.class, "gui").set(minecraft, gui);
            field(Hud.class, "isHidden").setBoolean(hud, false);

            var cam = newCamera(lvl);
            var renderer = make(GameRenderer.class);
            field(GameRenderer.class, "mainCamera").set(renderer, cam);
            field(GameRenderer.class, "minecraft").set(renderer, minecraft);
            field(GameRenderer.class, "renderBlockOutline").setBoolean(renderer, true);
            field(Minecraft.class, "gameRenderer").set(minecraft, renderer);

            var outlineObj = new JsonObject();
            for (var entry : List.of(Map.entry("local_player", (Entity) player), Map.entry("interaction", (Entity) interaction), Map.entry("creeper", (Entity) creeper))) {
                cam.setEntity(entry.getValue());
                outlineObj.addProperty(entry.getKey(), (boolean) shouldOutline.invoke(renderer));
            }
            out.add("block_outline", outlineObj);
        }

        // --- 4. Fog environments (lava, powder snow, water, blindness) + sky blocking ---
        {
            var lvl = newLevel();
            var player = newPlayer(lvl, 1, 0.5, 64.0, 0.5);
            player.spectatorFlag = true;
            player.waterVision = 0.25F;
            @SuppressWarnings("unchecked")
            var effects = (Map<Holder<net.minecraft.world.effect.MobEffect>, MobEffectInstance>) field(LivingEntity.class, "activeEffects").get(player);
            effects.put(MobEffects.BLINDNESS, new MobEffectInstance(MobEffects.BLINDNESS, 100, 0));
            effects.put(MobEffects.FIRE_RESISTANCE, new MobEffectInstance(MobEffects.FIRE_RESISTANCE, 100, 0));
            effects.put(MobEffects.NIGHT_VISION, new MobEffectInstance(MobEffects.NIGHT_VISION, 300, 0));
            var interaction = (Interaction) spawnViaPacket(lvl, "minecraft:interaction", 2, 0.5, 300.0, 0.5, 0.0F, 0.0F, 0.0F);
            minecraft.player = player;
            minecraft.level = lvl;

            var cam = newCamera(lvl);
            var delta = new FixedDelta(0.5F, 0.5F, 1.0F);
            var lavaEnv = new LavaFogEnvironment();
            var snowEnv = new PowderedSnowFogEnvironment();
            var waterEnv = new WaterFogEnvironment();
            var blindEnv = new BlindnessFogEnvironment();
            float renderDistBlocks = 128.0F;

            var fogObj = new JsonObject();
            for (var entry : List.of(Map.entry("spectator_local_player", (Entity) player), Map.entry("interaction", (Entity) interaction))) {
                cam.setEntity(entry.getValue());
                var row = new JsonObject();

                var lavaFog = new FogData();
                lavaEnv.setupFog(lavaFog, cam, lvl, renderDistBlocks, delta);
                row.addProperty("lava_start_bits", fb(lavaFog.environmentalStart));
                row.addProperty("lava_end_bits", fb(lavaFog.environmentalEnd));

                var snowFog = new FogData();
                snowEnv.setupFog(snowFog, cam, lvl, renderDistBlocks, delta);
                row.addProperty("powder_snow_start_bits", fb(snowFog.environmentalStart));
                row.addProperty("powder_snow_end_bits", fb(snowFog.environmentalEnd));

                var waterFog = new FogData();
                waterEnv.setupFog(waterFog, cam, lvl, renderDistBlocks, delta);
                row.addProperty("water_start_bits", fb(waterFog.environmentalStart));
                row.addProperty("water_end_bits", fb(waterFog.environmentalEnd));

                row.addProperty("blindness_applicable", blindEnv.isApplicable(FogType.ATMOSPHERIC, entry.getValue()));

                var cs = new CameraRenderState();
                cam.extractRenderState(cs, delta);
                row.addProperty("does_mob_effect_block_sky", cs.entityRenderState.doesMobEffectBlockSky);

                fogObj.add(entry.getKey(), row);
            }
            // Also non-spectator local player with fire resistance in lava.
            player.spectatorFlag = false;
            cam.setEntity(player);
            var fireResLava = new FogData();
            lavaEnv.setupFog(fireResLava, cam, lvl, renderDistBlocks, delta);
            var frRow = new JsonObject();
            frRow.addProperty("lava_start_bits", fb(fireResLava.environmentalStart));
            frRow.addProperty("lava_end_bits", fb(fireResLava.environmentalEnd));
            fogObj.add("fire_resistance_local_player", frRow);

            out.add("fog", fogObj);
        }

        // --- 5. Dark disc (reads minecraft.player, NOT camera) ---
        {
            var lvl = newLevel();
            var sky = make(SkyRenderer.class);
            var darkObj = new JsonObject();

            var pLow = newPlayer(lvl, 1, 0.5, 50.0, 0.5);
            minecraft.player = pLow;
            darkObj.addProperty("player_below_horizon_dry", (boolean) shouldDarkDisc.invoke(sky, 1.0F, lvl));

            pLow.underWaterFlag = true;
            darkObj.addProperty("player_below_horizon_underwater", (boolean) shouldDarkDisc.invoke(sky, 1.0F, lvl));

            var pHigh = newPlayer(lvl, 2, 0.5, 100.0, 0.5);
            minecraft.player = pHigh;
            darkObj.addProperty("player_above_horizon_dry", (boolean) shouldDarkDisc.invoke(sky, 1.0F, lvl));

            out.add("dark_disc", darkObj);
        }

        // --- 6. Vignette brightness & border distance from camera entity ---
        {
            var lvl = newLevel();
            lvl.defaultBrightness = 15;
            lvl.brightnessMap.put(new BlockPos(95, 301, 0), 0);
            lvl.worldBorder.setCenter(0.0, 0.0);
            lvl.worldBorder.setSize(200.0);
            lvl.worldBorder.setWarningBlocks(10);

            var player = newPlayer(lvl, 1, 0.5, 64.0, 0.5);
            var interaction = (Interaction) spawnViaPacket(lvl, "minecraft:interaction", 2, 95.5, 300.0, 0.5, 0.0F, 0.0F, 0.0F);
            interaction.getEntityData().assignValues(List.of(
                SynchedEntityData.DataValue.create(interactionWidthAccessor(), 0.1F),
                SynchedEntityData.DataValue.create(interactionHeightAccessor(), 1.9059F)
            ));

            var hudPlayer = make(Hud.class);
            hudPlayer.vignetteBrightness = 0.0F;
            updateVignette.invoke(hudPlayer, player);

            var hudCam = make(Hud.class);
            hudCam.vignetteBrightness = 0.0F;
            updateVignette.invoke(hudCam, interaction);

            var vigObj = new JsonObject();
            vigObj.addProperty("player_vignette_after_1_tick_bits", fb(hudPlayer.vignetteBrightness));
            vigObj.addProperty("interaction_vignette_after_1_tick_bits", fb(hudCam.vignetteBrightness));
            vigObj.addProperty("player_border_dist_bits", db(lvl.worldBorder.getDistanceToBorder(player)));
            vigObj.addProperty("interaction_border_dist_bits", db(lvl.worldBorder.getDistanceToBorder(interaction)));
            out.add("vignette", vigObj);
        }

        // --- 7. Streaming/particles, F3 debug entries, F3 gizmos (chunk borders + hitboxes), and spectator crosshair ---
        {
            var align = method(Camera.class, "alignWithEntity", float.class);
            var canCrosshair = method(Hud.class, "canRenderCrosshairForSpectator", HitResult.class);
            var lvl = newLevel();
            var biomeLookup = ParityData.access.lookupOrThrow(Registries.BIOME);
            lvl.chunkBiomes.put(((0L) << 32) ^ (0L & 0xffffffffL), biomeLookup.getOrThrow(Biomes.PLAINS));
            lvl.chunkBiomes.put(((6L) << 32) ^ (6L & 0xffffffffL), biomeLookup.getOrThrow(Biomes.DESERT));
            lvl.defaultBrightness = 15;
            lvl.brightnessMap.put(new BlockPos(96, 300, 96), 0);
            lvl.brightnessMap.put(new BlockPos(96, 301, 96), 0);

            var player = newPlayer(lvl, 1, 0.5, 64.0, 0.5);
            player.spectatorFlag = true;
            var interaction = (Interaction) spawnViaPacket(lvl, "minecraft:interaction", 31, 96.5, 300.0, 96.5, 0.0F, 0.0F, 0.0F);
            interaction.getEntityData().assignValues(List.of(
                SynchedEntityData.DataValue.create(interactionWidthAccessor(), 0.1F),
                SynchedEntityData.DataValue.create(interactionHeightAccessor(), 1.9059F)
            ));

            minecraft.player = player;
            minecraft.level = lvl;
            options.setCameraType(CameraType.FIRST_PERSON);
            var cam = newCamera(lvl);
            var renderer = make(GameRenderer.class);
            field(GameRenderer.class, "mainCamera").set(renderer, cam);
            field(GameRenderer.class, "minecraft").set(renderer, minecraft);
            field(Minecraft.class, "gameRenderer").set(minecraft, renderer);
            field(Minecraft.class, "deltaTracker").set(minecraft, new DeltaTracker.Timer(20.0F, 0L, f -> f));
            field(Minecraft.class, "levelExtractor").set(minecraft, make(StubLevelExtractor.class));

            cam.setEntity(interaction);
            for (int t = 0; t < 40; t++) cam.tick();
            align.invoke(cam, 1.0F);

            var streamObj = new JsonObject();
            SectionPos camSection = SectionPos.of(cam.blockPosition());
            SectionPos entitySection = SectionPos.of(interaction.blockPosition());
            var camChunk = new JsonArray();
            camChunk.add(camSection.x());
            camChunk.add(camSection.z());
            var entBlock = new JsonArray();
            entBlock.add(interaction.blockPosition().getX());
            entBlock.add(interaction.blockPosition().getY());
            entBlock.add(interaction.blockPosition().getZ());
            streamObj.add("camera_chunk", camChunk);
            streamObj.addProperty("entity_block_y", interaction.blockPosition().getY());
            streamObj.add("entity_block_pos", entBlock);
            streamObj.add("camera_pos_bits", vec3Bits(cam.position()));
            out.add("streaming_and_particles", streamObj);

            // Real DebugEntryPosition, DebugEntryBiome, DebugEntryLight
            List<String> posLines = new ArrayList<>();
            List<String> biomeLines = new ArrayList<>();
            List<String> lightLines = new ArrayList<>();
            DebugScreenDisplayer collectPos = new DebugScreenDisplayer() {
                @Override public void addPriorityLine(String s) { posLines.add(s); }
                @Override public void addLine(String s) { posLines.add(s); }
                @Override public void addToGroup(Identifier g, Collection<String> lines) { posLines.addAll(lines); }
                @Override public void addToGroup(Identifier g, String s) { posLines.add(s); }
            };
            DebugScreenDisplayer collectBiome = new DebugScreenDisplayer() {
                @Override public void addPriorityLine(String s) { biomeLines.add(s); }
                @Override public void addLine(String s) { biomeLines.add(s); }
                @Override public void addToGroup(Identifier g, Collection<String> lines) { biomeLines.addAll(lines); }
                @Override public void addToGroup(Identifier g, String s) { biomeLines.add(s); }
            };
            DebugScreenDisplayer collectLight = new DebugScreenDisplayer() {
                @Override public void addPriorityLine(String s) { lightLines.add(s); }
                @Override public void addLine(String s) { lightLines.add(s); }
                @Override public void addToGroup(Identifier g, Collection<String> lines) { lightLines.addAll(lines); }
                @Override public void addToGroup(Identifier g, String s) { lightLines.add(s); }
            };
            new DebugEntryPosition().display(collectPos, lvl, null, null);
            new DebugEntryBiome().display(collectBiome, lvl, null, null);
            new DebugEntryLight().display(collectLight, lvl, null, null);

            var debugObj = new JsonObject();
            var posArr = new JsonArray();
            posLines.forEach(posArr::add);
            debugObj.add("position_lines", posArr);
            debugObj.addProperty("biome_line", biomeLines.isEmpty() ? "" : biomeLines.get(0));
            debugObj.addProperty("light_line", lightLines.isEmpty() ? "" : lightLines.get(0));

            // Real ChunkBorderRenderer
            List<LineGizmo> borderLines = new ArrayList<>();
            try (var ignored = Gizmos.withCollector(g -> {
                if (g instanceof LineGizmo lg) borderLines.add(lg);
                return GizmoCollector.IGNORED;
            })) {
                new ChunkBorderRenderer(minecraft).emitGizmos(cam.position().x, cam.position().y, cam.position().z, null, null, 1.0F);
            }
            double minBx = Double.POSITIVE_INFINITY, maxBx = Double.NEGATIVE_INFINITY;
            double minBz = Double.POSITIVE_INFINITY, maxBz = Double.NEGATIVE_INFINITY;
            for (var lg : borderLines) {
                minBx = Math.min(minBx, Math.min(lg.start().x, lg.end().x));
                maxBx = Math.max(maxBx, Math.max(lg.start().x, lg.end().x));
                minBz = Math.min(minBz, Math.min(lg.start().z, lg.end().z));
                maxBz = Math.max(maxBz, Math.max(lg.start().z, lg.end().z));
            }
            var borderObj = new JsonObject();
            borderObj.addProperty("line_count", borderLines.size());
            borderObj.addProperty("min_x_bits", db(minBx));
            borderObj.addProperty("max_x_bits", db(maxBx));
            borderObj.addProperty("min_z_bits", db(minBz));
            borderObj.addProperty("max_z_bits", db(maxBz));
            debugObj.add("chunk_borders", borderObj);

            // Real EntityHitboxDebugRenderer in FIRST_PERSON while spectating interaction
            List<AABB> hitboxBoxes = new ArrayList<>();
            try (var ignored = Gizmos.withCollector(g -> {
                if (g instanceof CuboidGizmo cg) hitboxBoxes.add(cg.aabb());
                return GizmoCollector.IGNORED;
            })) {
                new EntityHitboxDebugRenderer(minecraft).emitGizmos(cam.position().x, cam.position().y, cam.position().z, null, null, 1.0F);
            }
            boolean hasPlayerBox = hitboxBoxes.stream().anyMatch(b -> b.minX >= 0.0 && b.maxX <= 1.0 && b.minY >= 63.9 && b.maxY <= 66.1);
            boolean hasInteractionBox = hitboxBoxes.stream().anyMatch(b -> b.minX >= 96.0 && b.maxX <= 97.0 && b.minY >= 299.9 && b.maxY <= 302.1);
            var hbObj = new JsonObject();
            hbObj.addProperty("player_box_emitted_in_first_person", hasPlayerBox);
            hbObj.addProperty("camera_interaction_box_emitted_in_first_person", hasInteractionBox);
            debugObj.add("entity_hitboxes", hbObj);
            out.add("debug", debugObj);

            // Real ProjectileUtil.getEntityHitResult + Hud.canRenderCrosshairForSpectator
            var crossLvl = newLevel();
            var crossPlayer = newPlayer(crossLvl, 1, 0.5, 64.0, 0.5);
            crossPlayer.spectatorFlag = true;
            crossPlayer.setYRot(0.0F);
            crossPlayer.setXRot(0.0F);
            minecraft.player = crossPlayer;
            minecraft.level = crossLvl;
            var hud = make(Hud.class);
            field(Hud.class, "minecraft").set(hud, minecraft);

            Entity cart = spawnViaPacket(crossLvl, "minecraft:chest_minecart", 40, 0.5, 65.2, 3.5, 0.0F, 0.0F, 0.0F);
            Vec3 eye = crossPlayer.getEyePosition(1.0F);
            Vec3 look = crossPlayer.getViewVector(1.0F);
            double reach = 4.5;
            Vec3 end = eye.add(look.scale(reach));
            AABB search = crossPlayer.getBoundingBox().expandTowards(look.scale(reach)).inflate(1.0, 1.0, 1.0);
            EntityHitResult hitCartOnly = ProjectileUtil.getEntityHitResult(
                crossPlayer, eye, end, search, e -> !e.isSpectator() && e.isPickable(), reach * reach
            );
            boolean crosshairCartOnly = (boolean) canCrosshair.invoke(hud, hitCartOnly);

            Interaction occluder = (Interaction) spawnViaPacket(crossLvl, "minecraft:interaction", 41, 0.5, 65.0, 2.0, 0.0F, 0.0F, 0.0F);
            EntityHitResult hitWithOccluder = ProjectileUtil.getEntityHitResult(
                crossPlayer, eye, end, search, e -> !e.isSpectator() && e.isPickable(), reach * reach
            );
            boolean crosshairWithOccluder = (boolean) canCrosshair.invoke(hud, hitWithOccluder);

            var crossObj = new JsonObject();
            crossObj.addProperty("cart_only_hit_id", hitCartOnly != null ? hitCartOnly.getEntity().getId() : -1);
            crossObj.addProperty("cart_only_crosshair", crosshairCartOnly);
            crossObj.addProperty("with_interaction_hit_id", hitWithOccluder != null ? hitWithOccluder.getEntity().getId() : -1);
            crossObj.addProperty("with_interaction_crosshair", crosshairWithOccluder);
            out.add("spectator_crosshair", crossObj);
        }

        return out;
    }

    static JsonObject recordCullFrustum() throws Exception {
        var out = new JsonObject();
        var createCullProj = method(Camera.class, "createProjectionMatrixForCulling");
        var prepareCull = method(Camera.class, "prepareCullFrustum", org.joml.Matrix4fc.class, Matrix4f.class, Vec3.class);
        var setRotation = method(Camera.class, "setRotation", float.class, float.class);
        var setPosition = method(Camera.class, "setPosition", double.class, double.class, double.class);

        var win = make(Window.class);
        win.setWidth(1280);
        win.setHeight(800);
        field(Minecraft.class, "window").set(minecraft, win);

        // Camera-relative AABBs probing 10/30/70/85 deg FOV and bob-sensitive edges (yaw=0 faces +Z).
        double[][] boxes = {
            // Directly ahead at z=+20.
            { -0.5, -0.5, 19.5, 0.5, 0.5, 20.5 },
            // Outside 30 deg horizontal FOV (x=10 at z=20), inside 70 deg.
            { 9.5, -0.5, 19.5, 10.5, 0.5, 20.5 },
            // Outside 30 deg vertical FOV (y=8 at z=20), inside 70 deg.
            { -0.5, 7.5, 19.5, 0.5, 8.5, 20.5 },
            // Outside 70 deg vertical FOV (y=16 at z=20), inside 85 deg.
            { -0.5, 15.5, 19.5, 0.5, 16.5, 20.5 },
            // Outside 85 deg vertical FOV (y=25 at z=20).
            { -0.5, 24.5, 19.5, 0.5, 25.5, 20.5 },
            // Behind camera at z=-20.
            { -0.5, -0.5, -20.5, 0.5, 0.5, -19.5 },
            // Near 70 deg horizontal edge (pushed outside if bob/spin tilts culling).
            { 21.0, -0.5, 19.5, 22.0, 0.5, 20.5 },
            // Near 70 deg vertical edge (pushed outside if bob/spin tilts culling).
            { -0.5, 12.7, 19.5, 0.5, 13.7, 20.5 },
            // Ahead of angled camera yaw=35, pitch=-15 at distance 20.
            { -11.5, 4.7, 15.3, -10.5, 5.7, 16.3 },
            // Offset ~22 deg from angled forward: outside 10 deg spyglass, inside 70 deg optionFov.
            { -18.0, 4.7, 12.0, -17.0, 5.7, 13.0 },
        };

        record Case(String name, float fov, int optionFov, float yaw, float pitch, float depthFar) {}
        List<Case> cases = List.of(
            new Case("zoomed_fov_30_opt_70", 30.0F, 70, 0.0F, 0.0F, 512.0F),
            new Case("widened_fov_85_opt_70", 85.0F, 70, 0.0F, 0.0F, 512.0F),
            new Case("spyglass_fov_10_opt_70_angled", 10.0F, 70, 35.0F, -15.0F, 512.0F)
        );

        var records = new JsonArray();
        for (Case tc : cases) {
            var lvl = newLevel();
            var cam = newCamera(lvl);
            options.fov().set(tc.optionFov);
            field(Camera.class, "fov").setFloat(cam, tc.fov);
            field(Camera.class, "depthFar").setFloat(cam, tc.depthFar);
            setPosition.invoke(cam, 0.0, 0.0, 0.0);
            setRotation.invoke(cam, tc.yaw, tc.pitch);

            Matrix4f viewRot = cam.getViewRotationMatrix(new Matrix4f());
            Matrix4f cullProj = (Matrix4f) createCullProj.invoke(cam);
            prepareCull.invoke(cam, viewRot, cullProj, cam.position());
            Frustum frustum = cam.getCullFrustum();

            var row = new JsonObject();
            row.addProperty("name", tc.name);
            row.addProperty("fov_bits", fb(tc.fov));
            row.addProperty("option_fov", tc.optionFov);
            row.addProperty("yaw_bits", fb(tc.yaw));
            row.addProperty("pitch_bits", fb(tc.pitch));
            row.addProperty("depth_far_bits", fb(tc.depthFar));
            row.addProperty("width", win.getWidth());
            row.addProperty("height", win.getHeight());
            row.add("cull_proj_bits", mat4Bits(cullProj));
            row.add("view_rot_bits", mat4Bits(viewRot));
            row.add("near_plane_bounds_bits", aabbBits(frustum.getNearPlaneBounds()));
            var vis = new JsonArray();
            for (double[] b : boxes) {
                var boxObj = new JsonObject();
                var aabb = new AABB(b[0], b[1], b[2], b[3], b[4], b[5]);
                boxObj.add("aabb_bits", aabbBits(aabb));
                boxObj.addProperty("visible", frustum.isVisible(aabb));
                vis.add(boxObj);
            }
            row.add("boxes", vis);
            records.add(row);
        }
        out.add("cases", records);
        return out;
    }

    public static void main(String[] args) throws Exception {
        ParityData.init();
        var u = Class.forName("sun.misc.Unsafe");
        unsafe = field(u, "theUnsafe").get(null);
        allocate = u.getMethod("allocateInstance", Class.class);

        installFakeGpuDevice();

        minecraft = make(Minecraft.class);
        field(Minecraft.class, "fixerUpper").set(minecraft, net.minecraft.util.datafix.DataFixers.getDataFixer());
        field(Minecraft.class, "instance").set(null, minecraft);
        field(Minecraft.class, "packetProcessor").set(minecraft, new PacketProcessor(Thread.currentThread()));
        options = new Options(minecraft, Files.createTempDirectory("camera_parity_opts").toFile());
        field(Minecraft.class, "options").set(minecraft, options);

        overworldDimType = ParityData.access
            .lookupOrThrow(Registries.DIMENSION_TYPE)
            .getValueOrThrow(net.minecraft.world.level.dimension.BuiltinDimensionTypes.OVERWORLD);

        var root = new JsonObject();
        root.add("entity_cameras", recordEntityCameras());
        root.add("interaction_lifecycle", recordInteractionLifecycle());
        root.add("camera_removal", recordCameraRemoval());
        root.add("camera_consumers", recordCameraConsumers());
        root.add("cull_frustum", recordCullFrustum());

        var gson = new GsonBuilder().setPrettyPrinting().serializeNulls().create();
        Files.writeString(Path.of(args[0]), gson.toJson(root) + "\n", StandardCharsets.UTF_8);
        System.exit(0);
    }
}
