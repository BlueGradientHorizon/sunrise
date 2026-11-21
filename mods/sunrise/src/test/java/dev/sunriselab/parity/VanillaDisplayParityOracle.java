package dev.sunriselab.parity;

import com.google.gson.*;
import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.blaze3d.vertex.CompactVectorArray;
import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.blaze3d.vertex.VertexConsumer;
import com.mojang.blaze3d.vertex.VertexSorting;
import com.mojang.math.Transformation;
import com.mojang.renderpearl.api.pipeline.DepthStencilState;
import com.mojang.renderpearl.api.pipeline.RenderPipeline;
import java.lang.reflect.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.*;
import java.util.concurrent.atomic.AtomicReference;
import net.minecraft.client.Camera;
import net.minecraft.client.CameraType;
import net.minecraft.client.Minecraft;
import net.minecraft.client.Options;
import net.minecraft.client.OptionInstance;
import net.minecraft.client.gui.Font;
import net.minecraft.client.renderer.GameRenderer;
import net.minecraft.client.renderer.LevelRenderer;
import net.minecraft.client.renderer.RenderPipelines;
import net.minecraft.client.renderer.Sheets;
import net.minecraft.client.renderer.SubmitNodeStorage;
import net.minecraft.client.renderer.SubmitNodeCollection;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.ViewArea;
import net.minecraft.client.renderer.block.BlockModelRenderState;
import net.minecraft.client.renderer.chunk.CompiledSectionMesh;
import net.minecraft.client.renderer.chunk.SectionMesh;
import net.minecraft.client.renderer.chunk.SectionRenderDispatcher;
import net.minecraft.client.renderer.culling.Frustum;
import net.minecraft.client.renderer.entity.DisplayRenderer;
import net.minecraft.client.renderer.entity.EntityRenderDispatcher;
import net.minecraft.client.renderer.entity.EntityRenderer;
import net.minecraft.client.renderer.entity.state.BlockDisplayEntityRenderState;
import net.minecraft.client.renderer.entity.state.DisplayEntityRenderState;
import net.minecraft.client.renderer.entity.state.EntityRenderState;
import net.minecraft.client.renderer.entity.state.ItemDisplayEntityRenderState;
import net.minecraft.client.renderer.entity.state.TextDisplayEntityRenderState;
import net.minecraft.client.renderer.extract.LevelExtractor;
import net.minecraft.client.renderer.feature.FlameFeatureRenderer;
import net.minecraft.client.renderer.feature.TextFeatureRenderer;
import net.minecraft.client.renderer.feature.phase.FeatureRenderPhase;
import net.minecraft.client.renderer.feature.phase.TranslucentFeatureRenderPhase;
import net.minecraft.client.renderer.feature.submit.SubmitNode;
import net.minecraft.client.renderer.rendertype.RenderType;
import net.minecraft.client.renderer.state.GameRenderState;
import net.minecraft.client.renderer.state.level.CameraRenderState;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import net.minecraft.core.BlockPos;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.TagParser;
import net.minecraft.network.chat.Component;
import net.minecraft.network.syncher.EntityDataAccessor;
import net.minecraft.network.syncher.SynchedEntityData;
import net.minecraft.resources.Identifier;
import net.minecraft.server.packs.resources.ResourceManager;
import net.minecraft.util.LightCoordsUtil;
import net.minecraft.util.ProblemReporter;
import net.minecraft.util.Util;
import net.minecraft.world.attribute.EnvironmentAttributeProbe;
import net.minecraft.world.entity.Display;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.Interaction;
import net.minecraft.world.entity.PositionPath;
import net.minecraft.world.item.ItemDisplayContext;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.level.BlockGetter;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.LightLayer;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.ChunkAccess;
import net.minecraft.world.level.dimension.DimensionType;
import net.minecraft.world.level.material.FluidState;
import net.minecraft.world.level.material.Fluids;
import net.minecraft.world.level.storage.TagValueInput;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.Vec3;
import org.joml.Matrix4f;
import org.joml.Quaternionf;
import org.joml.Quaternionfc;
import org.joml.Vector3f;
import org.joml.Vector3fc;

/**
 * Records real Minecraft 26.3 Display entity lifecycle, culling, billboard poses,
 * text submit phases/sorting, block-display render-type/culling properties,
 * name-tag/fire/invisibility behavior, teleport interpolation, and section
 * visibility gating directly from vanilla classes on JDK 25.
 */
public final class VanillaDisplayParityOracle {
    static Object unsafe;
    static Method allocate;

    @SuppressWarnings("unchecked")
    static <T> T alloc(Class<T> type) throws Exception {
        return (T) allocate.invoke(unsafe, type);
    }

    static Field findField(Class<?> owner, String name) throws Exception {
        Class<?> c = owner;
        while (c != null) {
            try {
                Field f = c.getDeclaredField(name);
                f.setAccessible(true);
                return f;
            } catch (NoSuchFieldException ignored) {
                c = c.getSuperclass();
            }
        }
        throw new NoSuchFieldException(owner.getName() + "." + name);
    }

    static void set(Object target, Class<?> owner, String name, Object value) throws Exception {
        findField(owner, name).set(target, value);
    }

    static Method findMethod(Class<?> owner, String name, Class<?>... params) throws Exception {
        Class<?> c = owner;
        while (c != null) {
            try {
                Method m = c.getDeclaredMethod(name, params);
                m.setAccessible(true);
                return m;
            } catch (NoSuchMethodException ignored) {
                c = c.getSuperclass();
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

    static JsonArray fArr(float... vs) {
        JsonArray a = new JsonArray();
        for (float v : vs) a.add(fb(v));
        return a;
    }

    static JsonArray dArr(double... vs) {
        JsonArray a = new JsonArray();
        for (double v : vs) a.add(db(v));
        return a;
    }

    static JsonArray mat4Bits(Matrix4f m) {
        float[] a = new float[16];
        m.get(a);
        return fArr(a);
    }

    @SuppressWarnings("unchecked")
    static <T> EntityDataAccessor<T> accessor(Class<?> owner, String name) throws Exception {
        return (EntityDataAccessor<T>) findField(owner, name).get(null);
    }

    static <T> SynchedEntityData.DataValue<T> dv(EntityDataAccessor<T> acc, T val) {
        return SynchedEntityData.DataValue.create(acc, val);
    }

    static void assign(Entity entity, SynchedEntityData.DataValue<?>... values) {
        entity.getEntityData().assignValues(List.of(values));
    }

    static void tickEntity(Entity entity) {
        entity.commonTick();
        entity.tick();
    }

    static final class StubChunk extends net.minecraft.world.level.chunk.LevelChunk {
        StubChunk() { super(null, null); }
        @Override
        public BlockState getBlockState(BlockPos pos) {
            return pos.getY() < 0 ? Blocks.STONE.defaultBlockState() : Blocks.AIR.defaultBlockState();
        }
    }

    static final class StubClientLevel extends net.minecraft.client.multiplayer.ClientLevel {
        DimensionType dimType;
        StubChunk chunk;
        net.minecraft.world.scores.Scoreboard scoreboard;
        int blockLight = 0;
        int skyLight = 0;

        StubClientLevel() { super(null, null, null, null, 0, 0, null, false, 0L, 0); }

        @Override public RegistryAccess registryAccess() { return ParityData.access; }
        @Override public net.minecraft.world.scores.Scoreboard getScoreboard() { return scoreboard; }
        @Override public int getBrightness(LightLayer layer, BlockPos pos) {
            return layer == LightLayer.BLOCK ? blockLight : skyLight;
        }
        @Override public int getMaxLocalRawBrightness(BlockPos pos) { return 15; }
        @Override public BlockState getBlockState(BlockPos pos) { return chunk.getBlockState(pos); }
        @Override public FluidState getFluidState(BlockPos pos) { return Fluids.EMPTY.defaultFluidState(); }
        @Override public ChunkAccess getChunk(BlockPos pos) { return chunk; }
        @Override public DimensionType dimensionType() { return dimType; }
        @Override public int getMinY() { return -64; }
        @Override public int getHeight() { return 384; }
    }

    static final class StubCamera extends Camera {
        @Override public net.minecraft.world.level.material.FogType getFluidInCamera() {
            return net.minecraft.world.level.material.FogType.NONE;
        }
    }

    static final class NoProbe extends EnvironmentAttributeProbe {
        @Override public void tick(net.minecraft.world.level.Level level, Vec3 position) { }
    }

    static StubClientLevel makeClientLevel() throws Exception {
        StubClientLevel level = alloc(StubClientLevel.class);
        set(level, net.minecraft.world.level.Level.class, "isClientSide", true);
        set(level, net.minecraft.world.level.Level.class, "registryAccess", ParityData.access);
        level.chunk = alloc(StubChunk.class);
        level.scoreboard = new net.minecraft.world.scores.Scoreboard();
        level.dimType = ParityData.access.lookupOrThrow(net.minecraft.core.registries.Registries.DIMENSION_TYPE)
            .getValueOrThrow(net.minecraft.world.level.dimension.BuiltinDimensionTypes.OVERWORLD);
        return level;
    }

    static EntityType<?> entityType(String name) {
        return BuiltInRegistries.ENTITY_TYPE.getValue(Identifier.parse(name));
    }

    @SuppressWarnings("unchecked")
    public static void main(String[] args) throws Exception {
        ParityData.init();
        var unsafeField = Class.forName("sun.misc.Unsafe").getDeclaredField("theUnsafe");
        unsafeField.setAccessible(true);
        unsafe = unsafeField.get(null);
        allocate = unsafe.getClass().getMethod("allocateInstance", Class.class);

        ResourceManager resources = ParityFont.resources();
        Font font = ParityFont.create(resources);
        net.minecraft.locale.Language.inject(
            net.minecraft.client.resources.language.ClientLanguage.loadFrom(resources, List.of("en_us"), false));

        // Wire minimal Minecraft singleton with Options(entityShadows=true) & GameRenderer
        GameRenderState gameRenderState = new GameRenderState();
        gameRenderState.optionsRenderState.backgroundForChatOnly = false;
        gameRenderState.optionsRenderState.textBackgroundOpacity = 0.25F;
        GameRenderer gameRenderer = alloc(GameRenderer.class);
        set(gameRenderer, GameRenderer.class, "gameRenderState", gameRenderState);
        Options options = alloc(Options.class);
        OptionInstance<Boolean> entityShadowsOpt = OptionInstance.createBoolean("options.entityShadows", true);
        set(options, Options.class, "entityShadows", entityShadowsOpt);
        Minecraft mc = alloc(Minecraft.class);
        set(mc, Minecraft.class, "gameRenderer", gameRenderer);
        set(mc, Minecraft.class, "options", options);
        set(null, Minecraft.class, "instance", mc);

        StubClientLevel level = makeClientLevel();
        Entity.setViewScale(1.0);

        JsonObject root = new JsonObject();
        JsonObject meta = new JsonObject();
        meta.addProperty("game_version", "26.3");
        meta.addProperty("java_version", Runtime.version().toString());
        meta.addProperty("oracle", "VanillaDisplayParityOracle");
        root.add("meta", meta);

        root.add("lifecycle", recordLifecycle(level));
        root.add("non_unit_quat_and_blend", recordNonUnitQuatAndBlend(level));
        root.add("live_culling_and_no_culling", recordLiveCullingAndNoCulling(level, font));
        root.add("billboard_camera_angles", recordBillboardCameraAngles(level, font));
        root.add("text_submit_phases_and_sorting", recordTextSubmitPhasesAndSorting(level, font));
        root.add("block_display_render_type_and_cull", recordBlockDisplayRenderTypeAndCull());
        root.add("display_entity_features", recordDisplayEntityFeatures(level, font));
        root.add("teleport_duration_and_pos_rot_lerp", recordTeleportDurationAndPosRotLerp(level));
        root.add("section_compiled_and_visible_gate", recordSectionCompiledAndVisibleGate(level, font));

        Path outPath = Path.of(args[0]);
        Files.createDirectories(outPath.getParent());
        Gson gson = new GsonBuilder().setPrettyPrinting().disableHtmlEscaping().create();
        Files.writeString(outPath, gson.toJson(root) + "\n", StandardCharsets.UTF_8);
    }

    static JsonObject recordLifecycle(StubClientLevel level) throws Exception {
        JsonObject out = new JsonObject();
        var A_START = accessor(Display.class, "DATA_TRANSFORMATION_INTERPOLATION_START_DELTA_TICKS_ID");
        var A_DUR = accessor(Display.class, "DATA_TRANSFORMATION_INTERPOLATION_DURATION_ID");
        var A_TRANS = accessor(Display.class, "DATA_TRANSLATION_ID");
        var A_SCALE = accessor(Display.class, "DATA_SCALE_ID");
        var A_BLOCK = accessor(Display.BlockDisplay.class, "DATA_BLOCK_STATE_ID");
        var A_ITEM = accessor(Display.ItemDisplay.class, "DATA_ITEM_STACK_ID");
        var A_ITEM_CTX = accessor(Display.ItemDisplay.class, "DATA_ITEM_DISPLAY_ID");
        var A_TEXT = accessor(Display.TextDisplay.class, "DATA_TEXT_ID");
        var A_SHADOW_R = accessor(Display.class, "DATA_SHADOW_RADIUS_ID");

        // 1. Pre-first-tick, idle tick before metadata, then first metadata snaps even with duration=4
        Display.BlockDisplay bd = new Display.BlockDisplay(entityType("minecraft:block_display"), level);
        JsonObject preTick = new JsonObject();
        preTick.addProperty("on_spawn_render_state_present", bd.renderState() != null);
        preTick.addProperty("on_spawn_block_state_present", bd.blockRenderState() != null);
        tickEntity(bd); // tick 1 with no metadata
        preTick.addProperty("after_empty_tick_render_state_present", bd.renderState() != null);
        preTick.addProperty("after_empty_tick_block_state_present", bd.blockRenderState() != null);
        assign(bd,
            dv(A_START, 0),
            dv(A_DUR, 4),
            dv(A_TRANS, new Vector3f(2.0F, 3.0F, 4.0F)),
            dv(A_SCALE, new Vector3f(2.0F, 2.0F, 2.0F)),
            dv(A_BLOCK, Blocks.STONE.defaultBlockState()));
        preTick.addProperty("after_metadata_before_tick_render_state_present", bd.renderState() != null);
        preTick.addProperty("after_metadata_before_tick_block_state_present", bd.blockRenderState() != null);
        tickEntity(bd); // tick 2: first metadata applied -> fresh (snaps)
        preTick.addProperty("after_first_meta_tick_render_state_present", bd.renderState() != null);
        preTick.addProperty("after_first_meta_tick_block_id", BuiltInRegistries.BLOCK.getKey(bd.blockRenderState().blockState().getBlock()).toString());
        float p0 = bd.calculateInterpolationProgress(0.0F);
        float pHalf = bd.calculateInterpolationProgress(0.5F);
        Vector3fc t0 = bd.renderState().transformation().get(p0).translation();
        Vector3fc tHalf = bd.renderState().transformation().get(pHalf).translation();
        Vector3fc s0 = bd.renderState().transformation().get(p0).scale();
        preTick.addProperty("tick2_progress_0_bits", fb(p0));
        preTick.addProperty("tick2_progress_half_bits", fb(pHalf));
        preTick.add("tick2_translation_at_0_bits", fArr(t0.x(), t0.y(), t0.z()));
        preTick.add("tick2_translation_at_half_bits", fArr(tHalf.x(), tHalf.y(), tHalf.z()));
        preTick.add("tick2_scale_at_0_bits", fArr(s0.x(), s0.y(), s0.z()));
        // Deferred swap on subsequent update
        assign(bd,
            dv(A_START, 0),
            dv(A_DUR, 4),
            dv(A_TRANS, new Vector3f(6.0F, 3.0F, 4.0F)),
            dv(A_BLOCK, BuiltInRegistries.BLOCK.getValue(Identifier.parse("minecraft:blue_terracotta")).defaultBlockState()));
        preTick.addProperty("after_swap_before_tick_block_id", BuiltInRegistries.BLOCK.getKey(bd.blockRenderState().blockState().getBlock()).toString());
        tickEntity(bd); // tick 3
        preTick.addProperty("after_swap_after_tick_block_id", BuiltInRegistries.BLOCK.getKey(bd.blockRenderState().blockState().getBlock()).toString());
        float p3 = bd.calculateInterpolationProgress(0.5F);
        Vector3fc t3 = bd.renderState().transformation().get(p3).translation();
        preTick.addProperty("swap_tick_progress_half_bits", fb(p3));
        preTick.add("swap_tick_translation_half_bits", fArr(t3.x(), t3.y(), t3.z()));
        out.add("block_display_pre_tick_and_swap", preTick);

        // 2. ItemDisplay pre-tick & deferred item/context swap
        Display.ItemDisplay idisp = new Display.ItemDisplay(entityType("minecraft:item_display"), level);
        JsonObject itemCase = new JsonObject();
        assign(idisp,
            dv(A_ITEM, new ItemStack(Items.END_CRYSTAL)),
            dv(A_ITEM_CTX, ItemDisplayContext.GUI.getId()));
        itemCase.addProperty("before_tick_has_item_state", idisp.itemRenderState() != null);
        tickEntity(idisp);
        itemCase.addProperty("after_tick1_item", BuiltInRegistries.ITEM.getKey(idisp.itemRenderState().itemStack().getItem()).toString());
        itemCase.addProperty("after_tick1_context", idisp.itemRenderState().itemTransform().name());
        assign(idisp,
            dv(A_ITEM, new ItemStack(Items.DIAMOND_SWORD)),
            dv(A_ITEM_CTX, ItemDisplayContext.FIXED.getId()));
        itemCase.addProperty("after_swap_before_tick_item", BuiltInRegistries.ITEM.getKey(idisp.itemRenderState().itemStack().getItem()).toString());
        itemCase.addProperty("after_swap_before_tick_context", idisp.itemRenderState().itemTransform().name());
        tickEntity(idisp);
        itemCase.addProperty("after_swap_after_tick_item", BuiltInRegistries.ITEM.getKey(idisp.itemRenderState().itemStack().getItem()).toString());
        itemCase.addProperty("after_swap_after_tick_context", idisp.itemRenderState().itemTransform().name());
        out.add("item_display_pre_tick_and_swap", itemCase);

        // 3. Initial start = Integer.MIN_VALUE: setting duration=10 without start_interpolation snaps (progress=1.0)
        Display.TextDisplay tdMin = new Display.TextDisplay(entityType("minecraft:text_display"), level);
        assign(tdMin, dv(A_TEXT, Component.literal("A")), dv(A_TRANS, new Vector3f(0.0F, 0.0F, 0.0F)));
        tickEntity(tdMin); // tick 1
        assign(tdMin, dv(A_DUR, 10), dv(A_TRANS, new Vector3f(10.0F, 0.0F, 0.0F))); // NO id 8!
        tickEntity(tdMin); // tick 2
        float pMin0 = tdMin.calculateInterpolationProgress(0.0F);
        float pMinHalf = tdMin.calculateInterpolationProgress(0.5F);
        Vector3fc tMin = tdMin.renderState().transformation().get(pMin0).translation();
        JsonObject startMin = new JsonObject();
        startMin.addProperty("progress_at_0_bits", fb(pMin0));
        startMin.addProperty("progress_at_half_bits", fb(pMinHalf));
        startMin.add("translation_at_0_bits", fArr(tMin.x(), tMin.y(), tMin.z()));
        out.add("initial_start_min_value", startMin);

        // 4. lastProgress preserved while culled and when duration <= 0
        Display.TextDisplay tdCulled = new Display.TextDisplay(entityType("minecraft:text_display"), level);
        assign(tdCulled, dv(A_TEXT, Component.literal("C")), dv(A_TRANS, new Vector3f(0.0F, 0.0F, 0.0F)));
        tickEntity(tdCulled); // tick 1: fresh at 0
        assign(tdCulled, dv(A_START, 0), dv(A_DUR, 4), dv(A_TRANS, new Vector3f(10.0F, 0.0F, 0.0F)));
        tickEntity(tdCulled); // tick 2: start=2, dur=4, from 0 -> 10
        tickEntity(tdCulled); // tick 3: elapsed=1 -> progress(0.0)=0.25
        float pVisible = tdCulled.calculateInterpolationProgress(0.0F);
        tickEntity(tdCulled); // tick 4 (culled: no calculateInterpolationProgress called!)
        // Now at tick 5 send new target (20,0,0), start=0, dur=4
        assign(tdCulled, dv(A_START, 0), dv(A_DUR, 4), dv(A_TRANS, new Vector3f(20.0F, 0.0F, 0.0F)));
        tickEntity(tdCulled); // tick 5: captures lastProgress=0.25 -> starts from 2.5, not 7.5!
        float pReTarget = tdCulled.calculateInterpolationProgress(0.0F);
        Vector3fc tReTarget = tdCulled.renderState().transformation().get(pReTarget).translation();
        JsonObject culledCase = new JsonObject();
        culledCase.addProperty("visible_progress_bits", fb(pVisible));
        culledCase.addProperty("retarget_progress_0_bits", fb(pReTarget));
        culledCase.add("retarget_start_translation_bits", fArr(tReTarget.x(), tReTarget.y(), tReTarget.z()));
        out.add("last_progress_while_culled", culledCase);

        // 5. duration == 0 and negative duration do not overwrite lastProgress
        Display.TextDisplay tdZero = new Display.TextDisplay(entityType("minecraft:text_display"), level);
        assign(tdZero, dv(A_TEXT, Component.literal("Z")), dv(A_TRANS, new Vector3f(0.0F, 0.0F, 0.0F)));
        tickEntity(tdZero); // tick 1
        assign(tdZero, dv(A_START, 0), dv(A_DUR, 4), dv(A_TRANS, new Vector3f(8.0F, 0.0F, 0.0F)));
        tickEntity(tdZero); // tick 2
        tickEntity(tdZero); // tick 3
        float pBeforeZero = tdZero.calculateInterpolationProgress(0.0F); // 0.25 -> lastProgress = 0.25
        assign(tdZero, dv(A_DUR, 0)); // only duration = 0, no render state update
        tickEntity(tdZero); // tick 4
        float pDuringZero = tdZero.calculateInterpolationProgress(0.5F); // returns 1.0, must NOT overwrite lastProgress!
        // Now chain a new interpolation with duration=4: must start from 0.25 * 8 = 2.0, NOT 1.0 * 8 = 8.0!
        assign(tdZero, dv(A_START, 0), dv(A_DUR, 4), dv(A_TRANS, new Vector3f(20.0F, 0.0F, 0.0F)));
        tickEntity(tdZero); // tick 5
        float pAfterZero = tdZero.calculateInterpolationProgress(0.0F);
        Vector3fc tAfterZero = tdZero.renderState().transformation().get(pAfterZero).translation();
        // Negative duration test:
        Display.TextDisplay tdNeg = new Display.TextDisplay(entityType("minecraft:text_display"), level);
        assign(tdNeg, dv(A_TEXT, Component.literal("N")), dv(A_TRANS, new Vector3f(0.0F, 0.0F, 0.0F)));
        tickEntity(tdNeg);
        assign(tdNeg, dv(A_START, 0), dv(A_DUR, -3), dv(A_TRANS, new Vector3f(9.0F, 0.0F, 0.0F)), dv(A_SHADOW_R, 2.0F));
        tickEntity(tdNeg);
        float pNeg = tdNeg.calculateInterpolationProgress(0.5F);
        Vector3fc tNeg = tdNeg.renderState().transformation().get(pNeg).translation();
        float srNeg = tdNeg.renderState().shadowRadius().get(pNeg);
        JsonObject durEdge = new JsonObject();
        durEdge.addProperty("progress_before_zero_bits", fb(pBeforeZero));
        durEdge.addProperty("progress_during_zero_bits", fb(pDuringZero));
        durEdge.addProperty("progress_after_zero_bits", fb(pAfterZero));
        durEdge.add("translation_after_zero_bits", fArr(tAfterZero.x(), tAfterZero.y(), tAfterZero.z()));
        durEdge.addProperty("neg_duration_progress_bits", fb(pNeg));
        durEdge.add("neg_duration_translation_bits", fArr(tNeg.x(), tNeg.y(), tNeg.z()));
        durEdge.addProperty("neg_duration_shadow_radius_bits", fb(srNeg));
        out.add("duration_zero_and_negative", durEdge);

        return out;
    }

    static JsonObject recordNonUnitQuatAndBlend(StubClientLevel level) throws Exception {
        JsonObject out = new JsonObject();
        var A_START = accessor(Display.class, "DATA_TRANSFORMATION_INTERPOLATION_START_DELTA_TICKS_ID");
        var A_DUR = accessor(Display.class, "DATA_TRANSFORMATION_INTERPOLATION_DURATION_ID");
        var A_LEFT = accessor(Display.class, "DATA_LEFT_ROTATION_ID");
        var A_RIGHT = accessor(Display.class, "DATA_RIGHT_ROTATION_ID");
        var A_TRANS = accessor(Display.class, "DATA_TRANSLATION_ID");

        Display.TextDisplay td = new Display.TextDisplay(entityType("minecraft:text_display"), level);
        assign(td, dv(A_LEFT, new Quaternionf(0.0F, 2.0F, 0.0F, 0.0F)), dv(A_RIGHT, new Quaternionf(1.0F, 1.0F, 1.0F, 1.0F)));
        tickEntity(td);
        assign(td, dv(A_START, 0), dv(A_DUR, 2),
            dv(A_LEFT, new Quaternionf(0.0F, 0.0F, 2.0F, 0.0F)),
            dv(A_RIGHT, new Quaternionf(0.0F, 1.0F, 0.0F, 1.0F)),
            dv(A_TRANS, new Vector3f(1.0F, 2.0F, 3.0F)));
        tickEntity(td);
        Transformation tAt1 = td.renderState().transformation().get(1.0F);
        Transformation tAtHalf = td.renderState().transformation().get(0.5F);
        out.add("at_1_left_rot_bits", fArr(tAt1.leftRotation().x(), tAt1.leftRotation().y(), tAt1.leftRotation().z(), tAt1.leftRotation().w()));
        out.add("at_1_right_rot_bits", fArr(tAt1.rightRotation().x(), tAt1.rightRotation().y(), tAt1.rightRotation().z(), tAt1.rightRotation().w()));
        out.add("at_1_matrix_bits", mat4Bits(tAt1.getMatrixCopy()));
        out.add("at_half_left_rot_bits", fArr(tAtHalf.leftRotation().x(), tAtHalf.leftRotation().y(), tAtHalf.leftRotation().z(), tAtHalf.leftRotation().w()));
        out.add("at_half_right_rot_bits", fArr(tAtHalf.rightRotation().x(), tAtHalf.rightRotation().y(), tAtHalf.rightRotation().z(), tAtHalf.rightRotation().w()));
        out.add("at_half_matrix_bits", mat4Bits(tAtHalf.getMatrixCopy()));
        return out;
    }

    static JsonObject recordLiveCullingAndNoCulling(StubClientLevel level, Font font) throws Exception {
        JsonObject out = new JsonObject();
        var A_VR = accessor(Display.class, "DATA_VIEW_RANGE_ID");
        var A_W = accessor(Display.class, "DATA_WIDTH_ID");
        var A_H = accessor(Display.class, "DATA_HEIGHT_ID");
        var A_TRANS = accessor(Display.class, "DATA_TRANSLATION_ID");
        var A_TEXT = accessor(Display.TextDisplay.class, "DATA_TEXT_ID");
        Entity.setViewScale(1.0);

        Display.TextDisplay td = new Display.TextDisplay(entityType("minecraft:text_display"), level);
        td.snapTo(0.5, 301.62, 0.5, 0.0F, 0.0F);
        assign(td, dv(A_TEXT, Component.literal("Menu")), dv(A_TRANS, new Vector3f(50.0F, 0.0F, -50.0F)));
        tickEntity(td);

        // Standalone view_range update (no render state id, no tick)
        assign(td, dv(A_VR, 0.03F)); // range = 1.92
        out.addProperty("live_vr_at_1_91", td.shouldRender(0.5 + 1.91, 301.62, 0.5));
        out.addProperty("live_vr_at_1_93", td.shouldRender(0.5 + 1.93, 301.62, 0.5));

        // Standalone width/height update (no render state id, no tick)
        out.addProperty("initial_affected_by_culling", td.affectedByCulling());
        assign(td, dv(A_W, 2.0F), dv(A_H, 3.0F));
        out.addProperty("after_wh_affected_by_culling", td.affectedByCulling());
        AABB box = td.getInterpolatedBoundingBox(1.0F);
        out.add("wh_culling_box_bits", dArr(box.minX, box.minY, box.minZ, box.maxX, box.maxY, box.maxZ));

        // Negative width/height AABB normalization
        assign(td, dv(A_W, -2.0F), dv(A_H, -3.0F));
        AABB negBox = td.getInterpolatedBoundingBox(1.0F);
        out.addProperty("neg_wh_affected_by_culling", td.affectedByCulling());
        out.add("neg_wh_culling_box_bits", dArr(negBox.minX, negBox.minY, negBox.minZ, negBox.maxX, negBox.maxY, negBox.maxZ));

        // Reset to noCulling (width=0) and test DisplayRenderer.shouldRender with a Frustum pointing AWAY from translation=(50,0,50)
        assign(td, dv(A_W, 0.0F), dv(A_H, 0.0F));
        DisplayRenderer.TextDisplayRenderer renderer = alloc(DisplayRenderer.TextDisplayRenderer.class);
        set(renderer, DisplayRenderer.TextDisplayRenderer.class, "font", font);
        // Camera at (0.5, 301.620015, 0.5) looking +Z (yaw=0, pitch=0), fov=70, aspect=16/10
        Matrix4f modelView = new Matrix4f().rotationXYZ(0.0F, (float) Math.PI, 0.0F);
        Matrix4f proj = new Matrix4f().setPerspective((float) Math.toRadians(70.0), 1280.0F / 800.0F, 0.05F, 256.0F);
        Frustum frustum = new Frustum(modelView, proj);
        frustum.prepare(0.5, 301.620015, 0.5);
        out.addProperty("no_culling_outside_frustum_should_render", renderer.shouldRender(td, frustum, 0.5, 301.620015, 0.5, 1.0F));

        // With finite box behind camera, shouldRender is false
        td.snapTo(0.5, 301.62, -1.0, 0.0F, 0.0F); // behind +Z camera, within 1.92 blocks
        assign(td, dv(A_W, 0.2F), dv(A_H, 0.2F));
        out.addProperty("finite_box_behind_camera_should_render", renderer.shouldRender(td, frustum, 0.5, 301.620015, 0.5, 1.0F));

        // Uninterpolated position used for distance check: xOld=0.5 (dist 0), x=2.5 (dist 2.0 >= 1.92)
        Display.TextDisplay tdMoving = new Display.TextDisplay(entityType("minecraft:text_display"), level);
        tdMoving.snapTo(0.5, 301.62, 0.5, 0.0F, 0.0F);
        assign(tdMoving, dv(A_VR, 0.03F));
        tdMoving.setOldPosAndRot();
        tdMoving.setPos(2.5, 301.62, 0.5);
        out.addProperty("moving_uninterpolated_outside_range_partial_0",
            renderer.shouldRender(tdMoving, frustum, 0.5, 301.62, 0.5, 0.0F));

        // Reverse: xOld=2.5 (dist 2.0 >= 1.92), x=0.5 (dist 0 < 1.92)
        tdMoving.snapTo(2.5, 301.62, 0.5, 0.0F, 0.0F);
        tdMoving.setOldPosAndRot();
        tdMoving.setPos(0.5, 301.62, 0.5);
        out.addProperty("moving_uninterpolated_inside_range_partial_0",
            renderer.shouldRender(tdMoving, frustum, 0.5, 301.62, 0.5, 0.0F));

        return out;
    }

    static JsonObject recordBillboardCameraAngles(StubClientLevel level, Font font) throws Exception {
        JsonObject out = new JsonObject();
        var A_BILLBOARD = accessor(Display.class, "DATA_BILLBOARD_RENDER_CONSTRAINTS_ID");
        var A_TRANS = accessor(Display.class, "DATA_TRANSLATION_ID");
        var A_SCALE = accessor(Display.class, "DATA_SCALE_ID");
        var A_LEFT = accessor(Display.class, "DATA_LEFT_ROTATION_ID");
        var A_TEXT = accessor(Display.TextDisplay.class, "DATA_TEXT_ID");
        var A_INT_W = accessor(Interaction.class, "DATA_WIDTH_ID");
        var A_INT_H = accessor(Interaction.class, "DATA_HEIGHT_ID");

        // Camera entity: Interaction 83082 at (0.5, 300.0, 0.5), width=0.1, height=1.9059
        Interaction camEntity = new Interaction(entityType("minecraft:interaction"), level);
        camEntity.snapTo(0.5, 300.0, 0.5, 0.0F, 0.0F);
        assign(camEntity, dv(A_INT_W, 0.1F), dv(A_INT_H, 1.9059F));

        DisplayRenderer.TextDisplayRenderer renderer = alloc(DisplayRenderer.TextDisplayRenderer.class);
        set(renderer, DisplayRenderer.TextDisplayRenderer.class, "font", font);
        EntityRenderDispatcher dispatcher = alloc(EntityRenderDispatcher.class);
        set(renderer, EntityRenderer.class, "entityRenderDispatcher", dispatcher);
        set(renderer, DisplayRenderer.class, "entityRenderDispatcher", dispatcher);

        Method calcOri = findMethod(DisplayRenderer.class, "calculateOrientation", Display.RenderState.class, DisplayEntityRenderState.class, Quaternionf.class);

        JsonArray cases = new JsonArray();
        record CamSetup(String name, float camYaw, float camPitch, CameraType type, float entYaw, float entPitch) {}
        List<CamSetup> setups = List.of(
            new CamSetup("genpvp_fixed_interaction_cam_0_0", 0.0F, 0.0F, CameraType.FIRST_PERSON, 0.0F, 0.0F),
            new CamSetup("spectated_angled_first_person", 37.5F, -12.0F, CameraType.FIRST_PERSON, 15.0F, 25.0F),
            new CamSetup("spectated_angled_third_person_back", 37.5F, -12.0F, CameraType.THIRD_PERSON_BACK, 15.0F, 25.0F),
            new CamSetup("spectated_angled_third_person_front", 37.5F, -12.0F, CameraType.THIRD_PERSON_FRONT, 15.0F, 25.0F)
        );
        Method alignWithEntity = findMethod(Camera.class, "alignWithEntity", float.class);
        for (CamSetup s : setups) {
            camEntity.snapTo(0.5, 300.0, 0.5, s.camYaw(), s.camPitch());
            StubCamera camera = alloc(StubCamera.class);
            set(camera, Camera.class, "minecraft", Minecraft.getInstance());
            set(Minecraft.getInstance().options, Options.class, "cameraType", s.type());
            set(camera, Camera.class, "level", level);
            set(camera, Camera.class, "entity", camEntity);
            set(camera, Camera.class, "position", Vec3.ZERO);
            set(camera, Camera.class, "blockPosition", new BlockPos.MutableBlockPos());
            set(camera, Camera.class, "forwards", new Vector3f(0.0F, 0.0F, 1.0F));
            set(camera, Camera.class, "up", new Vector3f(0.0F, 1.0F, 0.0F));
            set(camera, Camera.class, "left", new Vector3f(1.0F, 0.0F, 0.0F));
            set(camera, Camera.class, "rotation", new Quaternionf());
            set(camera, Camera.class, "eyeHeight", camEntity.getEyeHeight());
            set(camera, Camera.class, "eyeHeightOld", camEntity.getEyeHeight());
            set(camera, Camera.class, "attributeProbe", alloc(NoProbe.class));
            alignWithEntity.invoke(camera, 1.0F);
            dispatcher.camera = camera;

            for (byte bb = 0; bb <= 3; bb++) {
                Display.TextDisplay td = new Display.TextDisplay(entityType("minecraft:text_display"), level);
                td.snapTo(0.5, 301.62, 0.5, s.entYaw(), s.entPitch());
                assign(td,
                    dv(A_TEXT, Component.literal(" ")),
                    dv(A_BILLBOARD, bb),
                    dv(A_TRANS, new Vector3f(-1.395F, 0.3885F, -4.2F)),
                    dv(A_SCALE, new Vector3f(0.525F, 0.525F, 0.525F)),
                    dv(A_LEFT, new Quaternionf(0.0F, 1.0F, 0.0F, 0.0F)));
                tickEntity(td);
                TextDisplayEntityRenderState st = renderer.createRenderState();
                renderer.extractRenderState(td, st, 1.0F);
                Quaternionf ori = (Quaternionf) calcOri.invoke(renderer, st.renderState, st, new Quaternionf());
                Matrix4f pose = new Matrix4f().rotate(ori).mul(st.renderState.transformation().get(st.interpolationProgress).getMatrix());
                JsonObject c = new JsonObject();
                c.addProperty("name", s.name());
                c.addProperty("billboard", bb);
                c.addProperty("camera_type", s.type().name());
                c.addProperty("camera_yRot_bits", fb(st.cameraYRot));
                c.addProperty("camera_xRot_bits", fb(st.cameraXRot));
                c.addProperty("entity_yRot_bits", fb(st.entityYRot));
                c.addProperty("entity_xRot_bits", fb(st.entityXRot));
                c.add("pose_bits", mat4Bits(pose));
                cases.add(c);
            }
        }
        out.add("cases", cases);
        return out;
    }

    static List<SubmitNode> drainPhase(FeatureRenderPhase<?> phase) {
        List<SubmitNode> list = new ArrayList<>();
        phase.sortInto(new FeatureRenderPhase.Output() {
            @Override
            public void accept(SubmitNode submit, boolean strictlyOrdered) {
                list.add(submit);
            }
            @Override
            public <S extends SubmitNode> void acceptFeatureGroup(
                net.minecraft.client.renderer.feature.FeatureRendererType<S> featureType,
                Collection<S> submits,
                boolean strictlyOrdered
            ) {
                list.addAll(submits);
            }
        });
        return list;
    }

    static JsonObject recordTextSubmitPhasesAndSorting(StubClientLevel level, Font font) throws Exception {
        JsonObject out = new JsonObject();
        var A_TEXT = accessor(Display.TextDisplay.class, "DATA_TEXT_ID");
        var A_BG = accessor(Display.TextDisplay.class, "DATA_BACKGROUND_COLOR_ID");
        var A_OPACITY = accessor(Display.TextDisplay.class, "DATA_TEXT_OPACITY_ID");
        var A_FLAGS = accessor(Display.TextDisplay.class, "DATA_STYLE_FLAGS_ID");
        var A_LINE_W = accessor(Display.TextDisplay.class, "DATA_LINE_WIDTH_ID");

        DisplayRenderer.TextDisplayRenderer renderer = alloc(DisplayRenderer.TextDisplayRenderer.class);
        set(renderer, DisplayRenderer.TextDisplayRenderer.class, "font", font);
        EntityRenderDispatcher dispatcher = alloc(EntityRenderDispatcher.class);
        StubCamera camera = alloc(StubCamera.class);
        set(camera, Camera.class, "position", Vec3.ZERO);
        dispatcher.camera = camera;
        set(renderer, EntityRenderer.class, "entityRenderDispatcher", dispatcher);
        set(renderer, DisplayRenderer.class, "entityRenderDispatcher", dispatcher);

        RenderSystem.isRenderingLevel = true;
        CameraRenderState camState = new CameraRenderState();

        record TextCase(String name, String text, int bg, byte opacity, byte flags, float z) {}
        List<TextCase> textCases = List.of(
            new TextCase("opaque_panel", " ", 0xFF120A24, (byte) -1, (byte) 0, -4.89F),
            new TextCase("translucent_panel_with_opaque_text", "1000 ELO", 0xA0101722, (byte) -1, (byte) 1, -4.20F),
            new TextCase("transparent_bg_opaque_label", "Kuyruk", 0x00000000, (byte) -1, (byte) 1, -4.20F),
            new TextCase("translucent_text_and_bg", "Fade", 0x80000000, (byte) 128, (byte) 0, -4.50F),
            new TextCase("see_through_near", "Near", 0x80000000, (byte) -1, (byte) 2, -2.0F),
            new TextCase("see_through_far", "Far", 0x80000000, (byte) -1, (byte) 2, -6.0F),
            new TextCase("see_through_tie_a", "TieA", 0x80000000, (byte) -1, (byte) 2, -4.0F),
            new TextCase("see_through_tie_b", "TieB", 0x80000000, (byte) -1, (byte) 2, -4.0F)
        );

        JsonArray routing = new JsonArray();
        for (boolean improved : new boolean[] {false, true}) {
            for (TextCase tc : textCases) {
                SubmitNodeStorage storage = new SubmitNodeStorage();
                storage.setUseImprovedTransparency(improved);
                Display.TextDisplay td = new Display.TextDisplay(entityType("minecraft:text_display"), level);
                assign(td,
                    dv(A_TEXT, Component.literal(tc.text())),
                    dv(A_BG, tc.bg()),
                    dv(A_OPACITY, tc.opacity()),
                    dv(A_FLAGS, tc.flags()),
                    dv(A_LINE_W, 4000));
                tickEntity(td);
                TextDisplayEntityRenderState st = renderer.createRenderState();
                renderer.extractRenderState(td, st, 1.0F);
                PoseStack pose = new PoseStack();
                pose.translate(0.0F, 0.0F, tc.z());
                renderer.submit(st, pose, storage, camState);
                SubmitNodeCollection col = storage.order(0);

                JsonObject entry = new JsonObject();
                entry.addProperty("name", tc.name());
                entry.addProperty("improved_transparency", improved);
                entry.add("solid", describeSubmits(drainPhase(col.solid)));
                entry.add("texts", describeSubmits(improved ? List.of() : drainPhase(col.texts)));
                entry.add("oit_translucent", describeSubmits(improved ? drainPhase(col.oitTranslucent) : List.of()));
                entry.add("see_through", describeSubmits(drainPhase(storage.seeThrough())));
                routing.add(entry);
            }
        }
        out.add("phase_routing", routing);

        // See-through submit sorting order (far to near, stable on ties)
        TranslucentFeatureRenderPhase seeThroughAll = new TranslucentFeatureRenderPhase();
        SubmitNodeCollection colAll = new SubmitNodeCollection(false, seeThroughAll);
        String[] stOrderNames = {"near_2", "tie_first_4", "far_6", "tie_second_4"};
        float[] stZs = {-2.0F, -4.0F, -6.0F, -4.0F};
        for (int i = 0; i < stOrderNames.length; i++) {
            PoseStack pose = new PoseStack();
            pose.translate(0.0F, 0.0F, stZs[i]);
            colAll.submitTextBackground(pose, -1, -1, 4, 9, 0xFF000000 | i, Font.DisplayMode.SEE_THROUGH, 0xF000F0);
        }
        JsonArray stSorted = new JsonArray();
        for (SubmitNode node : drainPhase(seeThroughAll)) {
            TextFeatureRenderer.Submit sub = (TextFeatureRenderer.Submit) node;
            var bg = (TextFeatureRenderer.Content.StandaloneBackground) sub.content();
            stSorted.add(stOrderNames[bg.color() & 0xFF]);
        }
        out.add("see_through_sorted_order", stSorted);

        // VertexSorting.DISTANCE_TO_ORIGIN on quad centroids (far to near, stable on ties)
        float[][] centroids = {
            {0.0F, 0.0F, 4.20025F}, // 0: translucent card (near)
            {0.0F, 0.0F, 4.89025F}, // 1: opaque backdrop (far)
            {3.0F, 4.0F, 0.0F},     // 2: dist 5 (tie A)
            {4.0F, 3.0F, 0.0F},     // 3: dist 5 (tie B)
            {0.0F, 0.0F, 4.85080F}, // 4: mid backdrop
        };
        CompactVectorArray cva = new CompactVectorArray(centroids.length);
        for (int i = 0; i < centroids.length; i++) {
            cva.set(i, centroids[i][0], centroids[i][1], centroids[i][2]);
        }
        int[] sortedIdx = VertexSorting.DISTANCE_TO_ORIGIN.sort(cva);
        JsonArray quadOrder = new JsonArray();
        for (int idx : sortedIdx) quadOrder.add(idx);
        out.add("vertex_sorting_distance_to_origin_order", quadOrder);

        // Pipeline depth bias & sortOnUpload check
        JsonObject pipelines = new JsonObject();
        DepthStencilState classicDs = RenderPipelines.TEXT_POLYGON_OFFSET.getDepthStencilState();
        DepthStencilState oitDs = RenderPipelines.OIT_TEXT_POLYGON_OFFSET.accumulatePipeline().getDepthStencilState();
        pipelines.addProperty("text_polygon_offset_write_depth", classicDs.writeDepth());
        pipelines.addProperty("text_polygon_offset_bias_scale_bits", fb(classicDs.depthBiasScaleFactor()));
        pipelines.addProperty("text_polygon_offset_bias_constant_bits", fb(classicDs.depthBiasConstant()));
        pipelines.addProperty("text_polygon_offset_cull", RenderPipelines.TEXT_POLYGON_OFFSET.isCull());
        pipelines.addProperty("oit_text_polygon_offset_write_depth", oitDs.writeDepth());
        pipelines.addProperty("oit_text_polygon_offset_bias_scale_bits", fb(oitDs.depthBiasScaleFactor()));
        pipelines.addProperty("oit_text_polygon_offset_bias_constant_bits", fb(oitDs.depthBiasConstant()));
        pipelines.addProperty("oit_text_polygon_offset_cull", RenderPipelines.OIT_TEXT_POLYGON_OFFSET.accumulatePipeline().isCull());
        RenderType textPoRt = net.minecraft.client.renderer.rendertype.RenderTypes.textPolygonOffset(Identifier.withDefaultNamespace("textures/font/ascii.png"));
        RenderType textStRt = net.minecraft.client.renderer.rendertype.RenderTypes.textSeeThrough(Identifier.withDefaultNamespace("textures/font/ascii.png"));
        pipelines.addProperty("rt_text_polygon_offset_sort_on_upload", textPoRt.sortOnUpload());
        pipelines.addProperty("rt_text_see_through_sort_on_upload", textStRt.sortOnUpload());
        out.add("pipelines", pipelines);

        RenderSystem.isRenderingLevel = false;
        return out;
    }

    static JsonArray describeSubmits(List<SubmitNode> nodes) {
        JsonArray arr = new JsonArray();
        for (SubmitNode n : nodes) {
            if (n instanceof TextFeatureRenderer.Submit ts) {
                arr.add(ts.content() instanceof TextFeatureRenderer.Content.StandaloneBackground
                    ? "background:" + ts.displayMode().name()
                    : "text:" + ts.displayMode().name());
            } else {
                arr.add(n.featureType().toString());
            }
        }
        return arr;
    }

    static JsonObject recordBlockDisplayRenderTypeAndCull() throws Exception {
        JsonObject out = new JsonObject();
        RenderType cutout = Sheets.cutoutBlockItemSheet();
        RenderType translucent = Sheets.translucentBlockItemSheet();
        out.addProperty("cutout_cull", cutout.pipeline().isCull());
        out.addProperty("cutout_sort_on_upload", cutout.sortOnUpload());
        out.addProperty("cutout_has_blending", cutout.hasBlending());
        out.addProperty("translucent_cull", translucent.pipeline().isCull());
        out.addProperty("translucent_sort_on_upload", translucent.sortOnUpload());
        out.addProperty("translucent_has_blending", translucent.hasBlending());

        // BlockModelRenderState.setupModel chooses translucent vs cutout sheet
        Field rtField = findField(BlockModelRenderState.class, "renderType");
        BlockModelRenderState opaqueState = new BlockModelRenderState();
        opaqueState.setupModel(new Matrix4f(), false);
        out.addProperty("setup_model_opaque_is_cutout_sheet", rtField.get(opaqueState) == cutout);
        BlockModelRenderState transState = new BlockModelRenderState();
        transState.setupModel(new Matrix4f(), true);
        out.addProperty("setup_model_translucent_is_translucent_sheet", rtField.get(transState) == translucent);

        // Block-level light emission (BlockModelResolver)
        JsonObject emissions = new JsonObject();
        for (String name : new String[] {
            "minecraft:black_concrete", "minecraft:blue_terracotta", "minecraft:blue_stained_glass",
            "minecraft:ice", "minecraft:slime_block", "minecraft:glowstone", "minecraft:sea_lantern", "minecraft:torch"
        }) {
            Block b = BuiltInRegistries.BLOCK.getValue(Identifier.parse(name));
            BlockState bs = b.defaultBlockState();
            JsonObject info = new JsonObject();
            info.addProperty("light_emission", bs.getLightEmission());
            info.addProperty("packed_block_light_coords", LightCoordsUtil.pack(bs.getLightEmission(), 0));
            emissions.add(BuiltInRegistries.BLOCK.getKey(b).toString(), info);
        }
        out.add("blocks", emissions);
        return out;
    }

    static JsonObject recordDisplayEntityFeatures(StubClientLevel level, Font font) throws Exception {
        JsonObject out = new JsonObject();
        var A_FLAGS = accessor(Entity.class, "DATA_SHARED_FLAGS_ID");
        var A_NAME = accessor(Entity.class, "DATA_CUSTOM_NAME");
        var A_NAME_VIS = accessor(Entity.class, "DATA_CUSTOM_NAME_VISIBLE");
        var A_BRIGHT = accessor(Display.class, "DATA_BRIGHTNESS_OVERRIDE_ID");
        var A_SHADOW_R = accessor(Display.class, "DATA_SHADOW_RADIUS_ID");
        var A_SHADOW_S = accessor(Display.class, "DATA_SHADOW_STRENGTH_ID");
        var A_TEXT = accessor(Display.TextDisplay.class, "DATA_TEXT_ID");
        var A_BLOCK = accessor(Display.BlockDisplay.class, "DATA_BLOCK_STATE_ID");

        DisplayRenderer.TextDisplayRenderer renderer = alloc(DisplayRenderer.TextDisplayRenderer.class);
        set(renderer, DisplayRenderer.TextDisplayRenderer.class, "font", font);
        EntityRenderDispatcher dispatcher = alloc(EntityRenderDispatcher.class);
        StubCamera camera = alloc(StubCamera.class);
        set(camera, Camera.class, "position", new Vec3(0.0, 0.0, 5.0));
        dispatcher.camera = camera;
        set(renderer, EntityRenderer.class, "entityRenderDispatcher", dispatcher);
        set(renderer, DisplayRenderer.class, "entityRenderDispatcher", dispatcher);

        // 1. Name tags on TextDisplay
        Display.TextDisplay td = new Display.TextDisplay(entityType("minecraft:text_display"), level);
        td.snapTo(0.0, 0.0, 0.0, 0.0F, 0.0F);
        assign(td,
            dv(A_NAME, Optional.of(Component.literal("DisplayTag"))),
            dv(A_NAME_VIS, true));
        // Before first tick: renderState is null -> DisplayRenderer.submit does NOT submit name tag
        TextDisplayEntityRenderState stPre = renderer.createRenderState(td, 1.0F);
        List<String> preSubmits = captureSubmitCalls(renderer, stPre);
        JsonObject nameTags = new JsonObject();
        nameTags.addProperty("pre_tick_extracted_name", stPre.nameTag != null ? stPre.nameTag.getString() : null);
        nameTags.addProperty("pre_tick_submitted_name_tag", preSubmits.contains("submitNameTag"));

        // After first tick with empty text (renderState != null && textRenderState != null -> hasSubState() == true!)
        assign(td, dv(A_TEXT, Component.empty()));
        tickEntity(td);
        TextDisplayEntityRenderState stEmpty = renderer.createRenderState(td, 1.0F);
        List<String> emptySubmits = captureSubmitCalls(renderer, stEmpty);
        nameTags.addProperty("post_tick_empty_text_has_sub_state", stEmpty.hasSubState());
        nameTags.addProperty("post_tick_empty_text_submitted_name_tag", emptySubmits.contains("submitNameTag"));
        nameTags.add("attachment_offset_bits", dArr(stEmpty.nameTagAttachment.x, stEmpty.nameTagAttachment.y, stEmpty.nameTagAttachment.z));
        // Distance >= 64 suppresses name tag
        set(camera, Camera.class, "position", new Vec3(0.0, 0.0, 65.0));
        TextDisplayEntityRenderState stFar = renderer.createRenderState(td, 1.0F);
        nameTags.addProperty("beyond_64_extracted_name", stFar.nameTag != null);
        // CustomNameVisible = false suppresses name tag when not crosshair pick
        set(camera, Camera.class, "position", new Vec3(0.0, 0.0, 5.0));
        assign(td, dv(A_NAME_VIS, false));
        TextDisplayEntityRenderState stHidden = renderer.createRenderState(td, 1.0F);
        nameTags.addProperty("name_visible_false_extracted_name", stHidden.nameTag != null);
        // Crosshair pick shows name even when CustomNameVisible = false
        dispatcher.crosshairPickEntity = td;
        TextDisplayEntityRenderState stPick = renderer.createRenderState(td, 1.0F);
        nameTags.addProperty("crosshair_pick_extracted_name", stPick.nameTag != null);
        dispatcher.crosshairPickEntity = null;
        out.add("name_tags", nameTags);

        // 2. Fire: block light 15 when brightness == -1, override wins when brightness != -1, 0 flame vertices (0x0 bb)
        Method getBlockLight = findMethod(DisplayRenderer.class, "getBlockLightLevel", Display.class, BlockPos.class);
        Display.BlockDisplay bd = new Display.BlockDisplay(entityType("minecraft:block_display"), level);
        bd.snapTo(0.0, 0.0, 0.0, 0.0F, 0.0F);
        assign(bd, dv(A_FLAGS, (byte) 0x01), dv(A_BLOCK, Blocks.STONE.defaultBlockState()));
        tickEntity(bd);
        level.blockLight = 2;
        JsonObject fire = new JsonObject();
        fire.addProperty("is_on_fire", bd.isOnFire());
        fire.addProperty("display_fire_animation", bd.displayFireAnimation());
        fire.addProperty("bb_width_bits", fb(bd.getBbWidth()));
        fire.addProperty("bb_height_bits", fb(bd.getBbHeight()));
        fire.addProperty("block_light_no_override_on_fire", (int) getBlockLight.invoke(renderer, bd, BlockPos.ZERO));
        assign(bd, dv(A_BRIGHT, LightCoordsUtil.pack(3, 5)));
        tickEntity(bd);
        fire.addProperty("block_light_with_override_on_fire", (int) getBlockLight.invoke(renderer, bd, BlockPos.ZERO));
        // Run real FlameFeatureRenderer.prepare on DisplayEntityRenderState (bbWidth=0, bbHeight=0)
        TextDisplayEntityRenderState fireState = renderer.createRenderState(td, 1.0F);
        FlameFeatureRenderer flameRenderer = new FlameFeatureRenderer();
        Method flamePrepare = findMethod(FlameFeatureRenderer.class, "prepare",
            FlameFeatureRenderer.Submit.class, VertexConsumer.class, TextureAtlasSprite.class, TextureAtlasSprite.class);
        int[] flameVerts = {0};
        VertexConsumer counter = (VertexConsumer) Proxy.newProxyInstance(VertexConsumer.class.getClassLoader(),
            new Class<?>[] {VertexConsumer.class}, (proxy, method, a) -> {
                if (method.isDefault()) return InvocationHandler.invokeDefault(proxy, method, a);
                if (method.getName().equals("addVertex")) flameVerts[0]++;
                return proxy;
            });
        PoseStack ps = new PoseStack();
        flamePrepare.invoke(flameRenderer, new FlameFeatureRenderer.Submit(ps.last().copy(), fireState, new Quaternionf()), counter, null, null);
        fire.addProperty("flame_feature_vertex_count", flameVerts[0]);
        out.add("fire", fire);

        // 3. Invisibility suppresses display shadows; pre-first-tick has shadowRadius 0
        Display.TextDisplay tdShadow = new Display.TextDisplay(entityType("minecraft:text_display"), level);
        tdShadow.snapTo(0.5, 0.0, 0.5, 0.0F, 0.0F);
        assign(tdShadow, dv(A_SHADOW_R, 2.0F), dv(A_SHADOW_S, 1.0F), dv(A_TEXT, Component.literal("S")));
        JsonObject shadow = new JsonObject();
        TextDisplayEntityRenderState shPre = renderer.createRenderState(tdShadow, 1.0F);
        shadow.addProperty("pre_tick_shadow_radius_bits", fb(shPre.shadowRadius));
        shadow.addProperty("pre_tick_shadow_pieces", shPre.shadowPieces.size());
        tickEntity(tdShadow);
        TextDisplayEntityRenderState shVis = renderer.createRenderState(tdShadow, 1.0F);
        shadow.addProperty("visible_shadow_radius_bits", fb(shVis.shadowRadius));
        shadow.addProperty("visible_shadow_pieces", shVis.shadowPieces.size());
        assign(tdShadow, dv(A_FLAGS, (byte) 0x20)); // invisible
        TextDisplayEntityRenderState shInv = renderer.createRenderState(tdShadow, 1.0F);
        shadow.addProperty("invisible_shadow_radius_bits", fb(shInv.shadowRadius));
        shadow.addProperty("invisible_shadow_pieces", shInv.shadowPieces.size());
        out.add("invisibility_shadow", shadow);

        return out;
    }

    static List<String> captureSubmitCalls(DisplayRenderer.TextDisplayRenderer renderer, TextDisplayEntityRenderState st) {
        List<String> calls = new ArrayList<>();
        SubmitNodeCollector collector = (SubmitNodeCollector) Proxy.newProxyInstance(
            SubmitNodeCollector.class.getClassLoader(),
            new Class<?>[] {SubmitNodeCollector.class},
            (proxy, method, a) -> {
                calls.add(method.getName());
                return null;
            });
        renderer.submit(st, new PoseStack(), collector, new CameraRenderState());
        return calls;
    }

    static JsonObject recordTeleportDurationAndPosRotLerp(StubClientLevel level) throws Exception {
        JsonObject out = new JsonObject();
        var A_TP = accessor(Display.class, "DATA_POS_ROT_INTERPOLATION_DURATION_ID");

        // 1. Unclamped client synced teleport_duration = 100 (> 59)
        Display.TextDisplay td = new Display.TextDisplay(entityType("minecraft:text_display"), level);
        td.snapTo(0.0, 0.0, 0.0, 0.0F, 0.0F);
        assign(td, dv(A_TP, 100));
        td.getInterpolation().interpolateTo(PositionPath.of(new Vec3(100.0, 0.0, 0.0)), 0.0F, 0.0F, true);
        JsonObject unclamped = new JsonObject();
        for (int t = 1; t <= 100; t++) {
            tickEntity(td);
            if (t == 1 || t == 2 || t == 50 || t == 59 || t == 100) {
                unclamped.addProperty("tick_" + t + "_x_bits", db(td.getX()));
            }
        }
        out.add("client_unclamped_100", unclamped);

        // 2. NBT readAdditionalSaveData clamps teleport_duration to 0..59
        Display.TextDisplay tdNbt = new Display.TextDisplay(entityType("minecraft:text_display"), level);
        CompoundTag tag = TagParser.parseCompoundFully("{teleport_duration:100}");
        var valueInput = TagValueInput.create(ProblemReporter.DISCARDING, ParityData.lookup, tag);
        Method readSave = findMethod(Display.class, "readAdditionalSaveData", net.minecraft.world.level.storage.ValueInput.class);
        readSave.invoke(tdNbt, valueInput);
        out.addProperty("nbt_clamped_100", (int) tdNbt.getEntityData().get(A_TP));

        // 3. Negative teleport_duration = -5: does NOT snap and does NOT interpolate
        Display.TextDisplay tdNeg = new Display.TextDisplay(entityType("minecraft:text_display"), level);
        tdNeg.snapTo(0.0, 0.0, 0.0, 0.0F, 0.0F);
        assign(tdNeg, dv(A_TP, -5));
        tdNeg.getInterpolation().interpolateTo(PositionPath.of(new Vec3(10.0, 0.0, 0.0)), 45.0F, 15.0F, true);
        JsonObject negTp = new JsonObject();
        negTp.addProperty("has_active_interpolation", tdNeg.getInterpolation().hasActiveInterpolation());
        negTp.addProperty("before_tick_x_bits", db(tdNeg.getX()));
        tickEntity(tdNeg);
        negTp.addProperty("after_tick_x_bits", db(tdNeg.getX()));
        out.add("negative_teleport_duration", negTp);

        // 4. Same-target restart suppression
        Display.TextDisplay tdSame = new Display.TextDisplay(entityType("minecraft:text_display"), level);
        tdSame.snapTo(0.0, 0.0, 0.0, 0.0F, 0.0F);
        assign(tdSame, dv(A_TP, 4));
        tdSame.getInterpolation().interpolateTo(PositionPath.of(new Vec3(4.0, 0.0, 0.0)), 0.0F, 0.0F, true);
        tickEntity(tdSame); // 1.0 (3 steps left)
        tickEntity(tdSame); // 2.0 (2 steps left)
        double xAt2 = tdSame.getX();
        // Repeat identical target: must NOT reset steps back to 4!
        tdSame.getInterpolation().interpolateTo(PositionPath.of(new Vec3(4.0, 0.0, 0.0)), 0.0F, 0.0F, true);
        tickEntity(tdSame); // should be 3.0, NOT 2.5!
        double xAt3 = tdSame.getX();
        JsonObject sameTarget = new JsonObject();
        sameTarget.addProperty("x_after_2_ticks_bits", db(xAt2));
        sameTarget.addProperty("x_after_repeat_and_tick3_bits", db(xAt3));
        out.add("same_target_no_restart", sameTarget);

        // 5. Yaw wrapped rotLerp vs Pitch linear lerp & setXRot clamp [-90, 90]
        Display.TextDisplay tdRot = new Display.TextDisplay(entityType("minecraft:text_display"), level);
        tdRot.snapTo(0.0, 0.0, 0.0, 350.0F, -80.0F);
        assign(tdRot, dv(A_TP, 2));
        tdRot.getInterpolation().interpolateTo(PositionPath.of(new Vec3(0.0, 0.0, 0.0)), 10.0F, 80.0F, true);
        tickEntity(tdRot);
        JsonObject rot = new JsonObject();
        rot.addProperty("step1_yRot_bits", fb(tdRot.getYRot()));
        rot.addProperty("step1_xRot_bits", fb(tdRot.getXRot()));
        rot.addProperty("partial_half_yRot_bits", fb(tdRot.getYRot(0.5F)));
        rot.addProperty("partial_half_xRot_bits", fb(tdRot.getXRot(0.5F)));
        tdRot.setXRot(120.0F);
        rot.addProperty("setXRot_120_bits", fb(tdRot.getXRot()));
        tdRot.setXRot(-120.0F);
        rot.addProperty("setXRot_neg120_bits", fb(tdRot.getXRot()));
        out.add("rotation_lerp_and_clamp", rot);

        return out;
    }

    static JsonObject recordSectionCompiledAndVisibleGate(StubClientLevel level, Font font) throws Exception {
        JsonObject out = new JsonObject();

        // Real LevelRenderer + ViewArea stub + RenderSection + LevelExtractor.isEntityVisible
        SectionRenderDispatcher.RenderSection section = alloc(SectionRenderDispatcher.RenderSection.class);
        AtomicReference<SectionMesh> meshRef = new AtomicReference<>(CompiledSectionMesh.UNCOMPILED);
        set(section, SectionRenderDispatcher.RenderSection.class, "sectionMesh", meshRef);

        SectionRenderDispatcher.RenderSection[] returnedSection = {section};
        StubViewArea stubViewArea = alloc(StubViewArea.class);
        stubViewArea.target = returnedSection;

        DisplayRenderer.TextDisplayRenderer tdRenderer = alloc(DisplayRenderer.TextDisplayRenderer.class);
        set(tdRenderer, DisplayRenderer.TextDisplayRenderer.class, "font", font);
        EntityRenderDispatcher erd = alloc(EntityRenderDispatcher.class);
        set(erd, EntityRenderDispatcher.class, "renderers", Map.of(entityType("minecraft:text_display"), tdRenderer));

        LevelRenderer lr = alloc(LevelRenderer.class);
        set(lr, LevelRenderer.class, "viewArea", stubViewArea);
        set(lr, LevelRenderer.class, "entityRenderDispatcher", erd);

        LevelExtractor extractor = alloc(LevelExtractor.class);
        set(extractor, LevelExtractor.class, "level", level);
        set(extractor, LevelExtractor.class, "levelRenderer", lr);
        set(extractor, LevelExtractor.class, "minecraft", Minecraft.getInstance());

        Display.TextDisplay td = new Display.TextDisplay(entityType("minecraft:text_display"), level);
        td.snapTo(0.5, 301.62, 0.5, 0.0F, 0.0F);
        Matrix4f id = new Matrix4f();
        Frustum frustum = new Frustum(id, id);
        frustum.prepare(0.5, 301.62, 0.5);

        long fadeMs = 1000L;
        // 1. Inside build height, UNCOMPILED -> false
        meshRef.set(CompiledSectionMesh.UNCOMPILED);
        set(section, SectionRenderDispatcher.RenderSection.class, "uploadedTime", Util.getMillis() - 5000L);
        out.addProperty("inside_build_uncompiled", extractor.isEntityVisible(td, frustum, 0.5, 301.62, 0.5, 1.0F, fadeMs));

        // 2. Inside build height, EMPTY (compiled air section at y=301), elapsed 200ms / 1000ms (vis 0.2 < 0.3) -> false
        meshRef.set(CompiledSectionMesh.EMPTY);
        set(section, SectionRenderDispatcher.RenderSection.class, "uploadedTime", Util.getMillis() - 200L);
        out.addProperty("inside_build_empty_fading_0_2", extractor.isEntityVisible(td, frustum, 0.5, 301.62, 0.5, 1.0F, fadeMs));

        // 3. Inside build height, EMPTY, elapsed 400ms / 1000ms (vis 0.4 >= 0.3) -> true
        set(section, SectionRenderDispatcher.RenderSection.class, "uploadedTime", Util.getMillis() - 400L);
        out.addProperty("inside_build_empty_fading_0_4", extractor.isEntityVisible(td, frustum, 0.5, 301.62, 0.5, 1.0F, fadeMs));

        // 4. Inside build height, EMPTY, fadeDuration = 0 -> true
        set(section, SectionRenderDispatcher.RenderSection.class, "uploadedTime", Util.getMillis());
        out.addProperty("inside_build_empty_fade_0", extractor.isEntityVisible(td, frustum, 0.5, 301.62, 0.5, 1.0F, 0L));

        // 5. Outside build height (y = 325 > 319), UNCOMPILED -> true
        meshRef.set(CompiledSectionMesh.UNCOMPILED);
        td.snapTo(0.5, 325.0, 0.5, 0.0F, 0.0F);
        out.addProperty("above_build_height_uncompiled", extractor.isEntityVisible(td, frustum, 0.5, 325.0, 0.5, 1.0F, fadeMs));

        // 6. Below build height (y = -70 < -64), UNCOMPILED -> true
        td.snapTo(0.5, -70.0, 0.5, 0.0F, 0.0F);
        out.addProperty("below_build_height_uncompiled", extractor.isEntityVisible(td, frustum, 0.5, -70.0, 0.5, 1.0F, fadeMs));

        return out;
    }

    static final class StubViewArea extends ViewArea {
        SectionRenderDispatcher.RenderSection[] target;
        StubViewArea() { super(null, 0, 0, 0, 0, 0, null); }
        @Override
        public SectionRenderDispatcher.RenderSection getRenderSectionAt(BlockPos pos) {
            return target[0];
        }
    }
}
