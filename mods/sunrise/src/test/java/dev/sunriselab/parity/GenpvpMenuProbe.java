package dev.sunriselab.parity;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonNull;
import com.google.gson.JsonObject;
import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.math.Transformation;
import com.mojang.serialization.JsonOps;
import java.io.BufferedWriter;
import java.io.IOException;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.Deque;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Queue;
import java.util.concurrent.ConcurrentLinkedQueue;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.function.Supplier;
import net.minecraft.SharedConstants;
import net.minecraft.client.Camera;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.Minecraft;
import net.minecraft.client.Screenshot;
import net.minecraft.client.gui.Font;
import net.minecraft.client.model.Model;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.client.multiplayer.PlayerInfo;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.OrderedSubmitNodeCollector;
import net.minecraft.client.renderer.block.MovingBlockRenderState;
import net.minecraft.client.renderer.block.dispatch.BlockStateModelPart;
import net.minecraft.client.renderer.chunk.SectionRenderDispatcher;
import net.minecraft.client.renderer.culling.Frustum;
import net.minecraft.client.renderer.entity.DisplayRenderer;
import net.minecraft.client.renderer.entity.EntityRenderDispatcher;
import net.minecraft.client.renderer.entity.EntityRenderer;
import net.minecraft.client.renderer.entity.state.DisplayEntityRenderState;
import net.minecraft.client.renderer.entity.state.EntityRenderState;
import net.minecraft.client.renderer.entity.state.TextDisplayEntityRenderState;
import net.minecraft.client.renderer.feature.ModelFeatureRenderer;
import net.minecraft.client.renderer.gizmos.DrawableGizmoPrimitives;
import net.minecraft.client.renderer.item.ItemStackRenderState;
import net.minecraft.client.renderer.rendertype.RenderType;
import net.minecraft.client.renderer.state.level.CameraRenderState;
import net.minecraft.client.renderer.state.level.LevelRenderState;
import net.minecraft.client.renderer.state.level.QuadParticleRenderState;
import net.minecraft.client.renderer.texture.UvMapping;
import net.minecraft.client.resources.model.geometry.ItemQuads;
import net.minecraft.commands.arguments.blocks.BlockStateParser;
import net.minecraft.core.SectionPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.network.PacketProcessor;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.ComponentSerialization;
import net.minecraft.util.FormattedCharSequence;
import net.minecraft.util.Util;
import net.minecraft.util.profiling.ProfilerFiller;
import net.minecraft.util.profiling.metrics.MetricCategory;
import net.minecraft.util.profiling.metrics.profiling.MetricsRecorder;
import net.minecraft.world.TickRateManager;
import net.minecraft.world.entity.Display;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.player.Abilities;
import net.minecraft.world.item.ItemDisplayContext;
import net.minecraft.world.phys.Vec3;
import net.minecraft.world.phys.shapes.VoxelShape;
import org.joml.Matrix4f;
import org.joml.Quaternionf;
import org.joml.Quaternionfc;
import org.joml.Vector3fc;
import org.jspecify.annotations.Nullable;

public final class GenpvpMenuProbe {
    private static final Gson COMPACT_GSON = new GsonBuilder().serializeNulls().disableHtmlEscaping().create();
    private static final Gson PRETTY_GSON = new GsonBuilder().serializeNulls().disableHtmlEscaping().setPrettyPrinting().create();

    private static final Field METRICS_RECORDER_FIELD;
    private static final Field PACKETS_QUEUE_FIELD;
    private static final Field LISTENER_FIELD;
    private static final Method DISPLAY_GET_VIEW_RANGE;
    private static final Method DISPLAY_GET_WIDTH;
    private static final Method DISPLAY_GET_HEIGHT;
    private static final Method DISPLAY_RENDERER_CALC_ORIENTATION;

    static {
        try {
            METRICS_RECORDER_FIELD = Minecraft.class.getDeclaredField("metricsRecorder");
            METRICS_RECORDER_FIELD.setAccessible(true);
            PACKETS_QUEUE_FIELD = PacketProcessor.class.getDeclaredField("packetsToBeHandled");
            PACKETS_QUEUE_FIELD.setAccessible(true);
            Class<?> entryClass = Class.forName("net.minecraft.network.PacketProcessor$ListenerAndPacket");
            LISTENER_FIELD = entryClass.getDeclaredField("listener");
            LISTENER_FIELD.setAccessible(true);
            DISPLAY_GET_VIEW_RANGE = Display.class.getDeclaredMethod("getViewRange");
            DISPLAY_GET_VIEW_RANGE.setAccessible(true);
            DISPLAY_GET_WIDTH = Display.class.getDeclaredMethod("getWidth");
            DISPLAY_GET_WIDTH.setAccessible(true);
            DISPLAY_GET_HEIGHT = Display.class.getDeclaredMethod("getHeight");
            DISPLAY_GET_HEIGHT.setAccessible(true);
            DISPLAY_RENDERER_CALC_ORIENTATION = DisplayRenderer.class.getDeclaredMethod(
                "calculateOrientation", Display.RenderState.class, DisplayEntityRenderState.class, Quaternionf.class);
            DISPLAY_RENDERER_CALC_ORIENTATION.setAccessible(true);
        } catch (ReflectiveOperationException e) {
            throw new ExceptionInInitializerError(e);
        }
    }

    private final Path settledOut;
    private final Path ticksOut;
    private final Path screenshotDir;
    private final @Nullable Path replayDoneFile;
    private final int settleTicks;
    private final int midReplayTick;
    private final String captureSha256;
    private final String witnessCommand;
    private final String javaVersion;

    private BufferedWriter ticksWriter;
    private boolean startedTicks;
    private int recordedTick;
    private final AtomicInteger pendingPacketsHandled = new AtomicInteger(0);
    private boolean seenCameraEntity;
    private boolean midReplayShotQueued;
    private boolean replayDoneObserved;
    private int ticksAfterReplayDone;

    private boolean captureSettledNextExtract;
    private boolean settledExtracted;
    private JsonObject settledJson;
    private final List<String> gaps = new ArrayList<>();

    private @Nullable String pendingScreenshotName;
    private final AtomicInteger screenshotsInFlight = new AtomicInteger(0);
    private boolean stopRequested;
    private int postSettledFrames;

    private GenpvpMenuProbe(
        Path settledOut,
        Path ticksOut,
        Path screenshotDir,
        @Nullable Path replayDoneFile,
        int settleTicks,
        int midReplayTick,
        String captureSha256,
        String witnessCommand,
        String javaVersion
    ) {
        this.settledOut = settledOut;
        this.ticksOut = ticksOut;
        this.screenshotDir = screenshotDir;
        this.replayDoneFile = replayDoneFile;
        this.settleTicks = settleTicks;
        this.midReplayTick = midReplayTick;
        this.captureSha256 = captureSha256;
        this.witnessCommand = witnessCommand;
        this.javaVersion = javaVersion;
    }

    public static void main(String[] args) throws Exception {
        String probeOut = System.getProperty("sunrise.probeOut", "");
        String settledProp = System.getProperty("sunrise.settledOut", "");
        Path settledOut;
        if (!settledProp.isEmpty()) {
            settledOut = Path.of(settledProp);
        } else if (!probeOut.isEmpty()) {
            Path p = Path.of(probeOut);
            settledOut = probeOut.endsWith(".json") ? p : p.resolve("settled.json");
        } else {
            throw new IllegalArgumentException("Set -Dsunrise.probeOut or -Dsunrise.settledOut");
        }
        String ticksProp = System.getProperty("sunrise.ticksOut", "");
        Path ticksOut = ticksProp.isEmpty() ? settledOut.resolveSibling("ticks.jsonl") : Path.of(ticksProp);
        Path screenshotDir = Path.of(System.getProperty("sunrise.screenshotDir", "/projects/.hz-tmp/genpvp-menu/witness"));
        String replayDoneProp = System.getProperty("sunrise.replayDoneFile", "");
        Path replayDoneFile = replayDoneProp.isEmpty() ? null : Path.of(replayDoneProp);
        int settleTicks = Integer.parseInt(System.getProperty("sunrise.settleTicks", "100"));
        int midReplayTick = Integer.parseInt(System.getProperty("sunrise.midReplayTick", "110"));
        String captureSha256 = System.getProperty("sunrise.captureSha256", "");
        String witnessCommand = System.getProperty("sunrise.witnessCommand", "");
        String javaVersion = System.getProperty("sunrise.javaVersion", System.getProperty("java.version", ""));

        Files.createDirectories(settledOut.getParent());
        Files.createDirectories(ticksOut.getParent());
        Files.createDirectories(screenshotDir);

        GenpvpMenuProbe probe = new GenpvpMenuProbe(
            settledOut, ticksOut, screenshotDir, replayDoneFile, settleTicks, midReplayTick,
            captureSha256, witnessCommand, javaVersion
        );
        probe.startAttacher();
        net.minecraft.client.main.Main.main(args);
    }

    private void startAttacher() {
        AtomicBoolean scheduled = new AtomicBoolean(false);
        Thread attacher = new Thread(() -> {
            while (!scheduled.get()) {
                try {
                    Minecraft mc = Minecraft.getInstance();
                    if (mc != null) {
                        scheduled.set(true);
                        mc.execute(() -> installHook(mc));
                        return;
                    }
                    Thread.sleep(5);
                } catch (InterruptedException e) {
                    return;
                }
            }
        }, "genpvp-menu-probe-attacher");
        attacher.setDaemon(true);
        attacher.start();
    }

    @SuppressWarnings("unchecked")
    private void installHook(Minecraft mc) {
        try {
            this.ticksWriter = Files.newBufferedWriter(
                this.ticksOut, StandardCharsets.UTF_8,
                StandardOpenOption.CREATE, StandardOpenOption.TRUNCATE_EXISTING, StandardOpenOption.WRITE
            );
            // Replace the PacketProcessor queue before connecting so every play packet drained on the game thread is counted.
            Queue<Object> oldQueue = (Queue<Object>) PACKETS_QUEUE_FIELD.get(mc.packetProcessor());
            CountingQueue<Object> countingQueue = new CountingQueue<>();
            if (oldQueue != null) {
                Object item;
                while ((item = oldQueue.poll()) != null) {
                    countingQueue.add(item);
                }
            }
            PACKETS_QUEUE_FIELD.set(mc.packetProcessor(), countingQueue);
            METRICS_RECORDER_FIELD.set(mc, new ProbeMetricsRecorder(mc));
            System.err.println("[GenpvpMenuProbe] Installed synchronous tick/frame hook");
        } catch (Throwable t) {
            t.printStackTrace(System.err);
            mc.stop();
        }
    }

    private void onClientTickEnd(Minecraft mc) {
        if (this.settledExtracted || this.stopRequested) {
            return;
        }
        if (mc.level == null || mc.player == null) {
            this.pendingPacketsHandled.set(0);
            return;
        }
        if (!this.startedTicks) {
            this.startedTicks = true;
            this.recordedTick = 0;
        }

        int packetsThisTick = this.pendingPacketsHandled.getAndSet(0);

        LocalPlayer player = mc.player;
        Entity camEntity = mc.getCameraEntity();
        String localMode = mc.gameMode == null ? null : mc.gameMode.getPlayerMode().getSerializedName();
        String infoMode = playerInfoGameMode(mc, player);

        JsonObject row = new JsonObject();
        row.addProperty("tick", this.recordedTick);
        row.addProperty("packets_handled", packetsThisTick);
        row.add("player_pos_bits", doubleBitsArray(player.getX(), player.getY(), player.getZ()));
        row.addProperty("on_ground", player.onGround());
        row.addProperty("no_physics", player.noPhysics);
        addNullableString(row, "local_game_mode", localMode);
        addNullableString(row, "player_info_game_mode", infoMode);
        row.addProperty("camera_entity_id", camEntity == null ? -1 : camEntity.getId());
        if (camEntity == null) {
            row.add("camera_entity_pos_bits", JsonNull.INSTANCE);
        } else {
            row.add("camera_entity_pos_bits", doubleBitsArray(camEntity.getX(), camEntity.getY(), camEntity.getZ()));
        }
        row.addProperty("player_yRot_bits", fbits(player.getYRot()));
        row.addProperty("player_xRot_bits", fbits(player.getXRot()));

        try {
            this.ticksWriter.write(COMPACT_GSON.toJson(row));
            this.ticksWriter.write('\n');
            this.ticksWriter.flush();
        } catch (IOException e) {
            throw new RuntimeException(e);
        }

        if (!this.seenCameraEntity && camEntity != null && camEntity != player) {
            this.seenCameraEntity = true;
            this.pendingScreenshotName = "camera_set.png";
            System.err.println("[GenpvpMenuProbe] Camera entity set at tick " + this.recordedTick + " -> id=" + camEntity.getId());
        }

        if (!this.midReplayShotQueued && this.seenCameraEntity && this.recordedTick >= this.midReplayTick && !this.replayDoneObserved) {
            this.midReplayShotQueued = true;
            if (this.pendingScreenshotName == null) {
                this.pendingScreenshotName = "mid_replay.png";
            }
            System.err.println("[GenpvpMenuProbe] Mid-replay screenshot queued at tick " + this.recordedTick);
        }

        if (!this.replayDoneObserved && this.replayDoneFile != null && Files.exists(this.replayDoneFile)) {
            if (!this.midReplayShotQueued && this.seenCameraEntity) {
                this.midReplayShotQueued = true;
                this.pendingScreenshotName = "mid_replay.png";
            }
            this.replayDoneObserved = true;
            this.ticksAfterReplayDone = 0;
            System.err.println("[GenpvpMenuProbe] Replay done observed at tick " + this.recordedTick);
        } else if (this.replayDoneObserved) {
            if (packetsThisTick > 0) {
                // Tail packets from the replay socket arrived on this tick; start the 100 quiet settle ticks after them.
                this.ticksAfterReplayDone = 0;
            } else {
                this.ticksAfterReplayDone++;
                if (this.ticksAfterReplayDone >= this.settleTicks) {
                    this.captureSettledNextExtract = true;
                    this.pendingScreenshotName = "settled.png";
                    System.err.println("[GenpvpMenuProbe] Settled at tick " + this.recordedTick + " (+" + this.ticksAfterReplayDone + " quiet ticks)");
                }
            }
        }

        this.recordedTick++;
    }

    private void onSampleDuringExtract(Minecraft mc) {
        if (!this.captureSettledNextExtract || this.settledExtracted) {
            return;
        }
        if (mc.level == null || mc.player == null) {
            return;
        }
        this.captureSettledNextExtract = false;
        this.settledExtracted = true;
        try {
            this.settledJson = buildSettledJson(mc);
            if (this.ticksWriter != null) {
                this.ticksWriter.flush();
                this.ticksWriter.close();
                this.ticksWriter = null;
            }
        } catch (Throwable t) {
            t.printStackTrace(System.err);
            mc.stop();
        }
    }

    private void onFrameEnd(Minecraft mc) {
        if (this.pendingScreenshotName != null && mc.gameRenderer != null && mc.gameRenderer.mainRenderTarget() != null) {
            String fileName = this.pendingScreenshotName;
            this.pendingScreenshotName = null;
            Path target = this.screenshotDir.resolve(fileName);
            this.screenshotsInFlight.incrementAndGet();
            Screenshot.takeScreenshot(mc.gameRenderer.mainRenderTarget(), image -> {
                try (image) {
                    image.writeToFile(target);
                    System.err.println("[GenpvpMenuProbe] Saved screenshot " + target);
                } catch (IOException e) {
                    e.printStackTrace(System.err);
                    this.gaps.add("screenshot write failed for " + fileName + ": " + e);
                } finally {
                    this.screenshotsInFlight.decrementAndGet();
                }
            });
        }

        if (this.settledExtracted && !this.stopRequested) {
            this.postSettledFrames++;
            if (this.screenshotsInFlight.get() == 0 && this.postSettledFrames >= 2) {
                this.stopRequested = true;
                try {
                    JsonObject meta = this.settledJson.getAsJsonObject("meta");
                    JsonArray gapsArr = new JsonArray();
                    for (String g : this.gaps) {
                        gapsArr.add(g);
                    }
                    meta.add("gaps", gapsArr);
                    Files.writeString(this.settledOut, PRETTY_GSON.toJson(this.settledJson) + "\n", StandardCharsets.UTF_8);
                    System.err.println("[GenpvpMenuProbe] Wrote " + this.settledOut);
                } catch (IOException e) {
                    e.printStackTrace(System.err);
                }
                mc.stop();
            }
        }
    }

    private JsonObject buildSettledJson(Minecraft mc) throws ReflectiveOperationException {
        LocalPlayer player = mc.player;
        Camera cam = mc.gameRenderer.mainCamera();
        LevelRenderState levelState = mc.gameRenderer.gameRenderState().levelRenderState;
        CameraRenderState camState = levelState.cameraRenderState;
        Entity camEntity = mc.getCameraEntity();
        EntityRenderDispatcher dispatcher = mc.getEntityRenderDispatcher();
        DeltaTracker deltaTracker = mc.getDeltaTracker();
        TickRateManager tickRateManager = mc.level.tickRateManager();
        Frustum cullFrustum = cam.getCullFrustum();
        Vec3 camPos = cam.position();
        double camX = camPos.x();
        double camY = camPos.y();
        double camZ = camPos.z();
        long chunkFadeDuration = Util.toMillis(mc.options.chunkSectionFadeInTime().get());

        JsonObject root = new JsonObject();

        JsonObject meta = new JsonObject();
        meta.addProperty("game_version", SharedConstants.getCurrentVersion().name());
        meta.addProperty("java_version", this.javaVersion);
        meta.addProperty("capture_sha256", this.captureSha256);
        JsonObject opts = new JsonObject();
        opts.addProperty("width", mc.getWindow().getWidth());
        opts.addProperty("height", mc.getWindow().getHeight());
        opts.addProperty("renderDistance", mc.options.renderDistance().get());
        opts.addProperty("effectiveRenderDistance", mc.options.getEffectiveRenderDistance());
        opts.addProperty("simulationDistance", mc.options.simulationDistance().get());
        opts.addProperty("guiScale", mc.options.guiScale().get());
        opts.addProperty("fov", mc.options.fov().get());
        opts.addProperty("improvedTransparency", mc.options.improvedTransparency().get());
        opts.addProperty("entityDistanceScaling", mc.options.entityDistanceScaling().get());
        opts.addProperty("chunkSectionFadeInTime", mc.options.chunkSectionFadeInTime().get());
        meta.add("options", opts);
        meta.addProperty("command", this.witnessCommand);
        meta.addProperty("settled_tick", this.recordedTick - 1);
        meta.addProperty("matrix_order", "column-major");
        JsonArray notes = new JsonArray();
        notes.add("Offline client run without launcher assets/indexes/34.json (unifont.zip); 5 Unihex glyphs (U+2196, U+25B0, U+25B1, U+25B8, U+25A6) in text displays 83100, 83103, 83246 use vanilla's 5px missing-glyph fallback advance.");
        notes.add("ticks.jsonl packets_handled counts play-state PacketProcessor tasks drained on the game thread (each bundle counts as 1 ClientboundBundlePacket task; Netty-thread keep_alive packets are not queued on PacketProcessor).");
        meta.add("notes", notes);
        meta.add("gaps", new JsonArray());
        root.add("meta", meta);

        JsonObject playerObj = new JsonObject();
        playerObj.addProperty("id", player.getId());
        playerObj.add("pos", doubleArray(player.getX(), player.getY(), player.getZ()));
        playerObj.add("pos_bits", doubleBitsArray(player.getX(), player.getY(), player.getZ()));
        playerObj.addProperty("yRot", player.getYRot());
        playerObj.addProperty("xRot", player.getXRot());
        playerObj.add("rot_bits", floatBitsArray(player.getYRot(), player.getXRot()));
        playerObj.addProperty("on_ground", player.onGround());
        playerObj.addProperty("no_physics", player.noPhysics);
        playerObj.addProperty("is_spectator", player.isSpectator());
        addNullableString(playerObj, "local_game_mode", mc.gameMode == null ? null : mc.gameMode.getPlayerMode().getSerializedName());
        addNullableString(playerObj, "player_info_game_mode", playerInfoGameMode(mc, player));
        Abilities ab = player.getAbilities();
        JsonObject abObj = new JsonObject();
        abObj.addProperty("flying", ab.flying);
        abObj.addProperty("may_fly", ab.mayfly);
        abObj.addProperty("invulnerable", ab.invulnerable);
        abObj.addProperty("instabuild", ab.instabuild);
        abObj.addProperty("fly_speed_bits", fbits(ab.getFlyingSpeed()));
        abObj.addProperty("walk_speed_bits", fbits(ab.getWalkingSpeed()));
        playerObj.add("abilities", abObj);
        root.add("player", playerObj);

        JsonObject camObj = new JsonObject();
        camObj.addProperty("entity_id", camEntity == null ? -1 : camEntity.getId());
        addNullableString(camObj, "entity_type", camEntity == null ? null : BuiltInRegistries.ENTITY_TYPE.getKey(camEntity.getType()).toString());
        if (camEntity != null) {
            camObj.add("entity_pos", doubleArray(camEntity.getX(), camEntity.getY(), camEntity.getZ()));
            camObj.add("entity_pos_bits", doubleBitsArray(camEntity.getX(), camEntity.getY(), camEntity.getZ()));
            camObj.addProperty("entity_eye_height_bits", fbits(camEntity.getEyeHeight()));
            camObj.addProperty("entity_yRot_bits", fbits(camEntity.getYRot()));
            camObj.addProperty("entity_xRot_bits", fbits(camEntity.getXRot()));
        } else {
            camObj.add("entity_pos", JsonNull.INSTANCE);
            camObj.add("entity_pos_bits", JsonNull.INSTANCE);
            camObj.add("entity_eye_height_bits", JsonNull.INSTANCE);
            camObj.add("entity_yRot_bits", JsonNull.INSTANCE);
            camObj.add("entity_xRot_bits", JsonNull.INSTANCE);
        }
        camObj.add("camera_pos", doubleArray(camX, camY, camZ));
        camObj.add("camera_pos_bits", doubleBitsArray(camX, camY, camZ));
        camObj.addProperty("camera_yRot_bits", fbits(cam.yRot()));
        camObj.addProperty("camera_xRot_bits", fbits(cam.xRot()));
        camObj.addProperty("detached", cam.isDetached());
        camObj.addProperty("partial_tick_bits", fbits(camState.cameraEntityPartialTicks));
        root.add("camera", camObj);

        JsonObject frameObj = new JsonObject();
        frameObj.addProperty("fov_bits", fbits(cam.getFov()));
        JsonArray visibleSectionsArr = new JsonArray();
        for (SectionRenderDispatcher.RenderSection section : mc.levelRenderer.visibleSections()) {
            long node = section.getSectionNode();
            JsonArray coord = new JsonArray();
            coord.add(SectionPos.x(node));
            coord.add(SectionPos.y(node));
            coord.add(SectionPos.z(node));
            visibleSectionsArr.add(coord);
        }
        frameObj.addProperty("visible_terrain_sections", visibleSectionsArr.size());
        frameObj.addProperty("rendered_terrain_sections", mc.levelExtractor.countRenderedSections());
        frameObj.add("visible_sections", visibleSectionsArr);
        frameObj.addProperty("rendered_entity_count", levelState.entityRenderStates.size());
        root.add("frame", frameObj);

        // Match LevelExtractor.extractVisibleEntities order 1:1 to pair each entity with its live EntityRenderState.
        Map<Integer, Integer> renderOrderById = new LinkedHashMap<>();
        Map<Integer, EntityRenderState> renderStateById = new LinkedHashMap<>();
        List<Entity> allEntities = new ArrayList<>();
        int nextOrder = 0;
        for (Entity entity : mc.level.entitiesForRendering()) {
            allEntities.add(entity);
            float entityPartialTicks = deltaTracker.getGameTimeDeltaPartialTick(!tickRateManager.isEntityFrozen(entity));
            boolean extracted = mc.levelExtractor.isEntityVisible(entity, cullFrustum, camX, camY, camZ, entityPartialTicks, chunkFadeDuration)
                && (entity != cam.entity() || cam.isDetached() || cam.entity() instanceof LivingEntity le && le.isSleeping())
                && (!(entity instanceof LocalPlayer) || cam.entity() == entity);
            if (extracted) {
                if (nextOrder < levelState.entityRenderStates.size()) {
                    renderOrderById.put(entity.getId(), nextOrder);
                    renderStateById.put(entity.getId(), levelState.entityRenderStates.get(nextOrder));
                }
                nextOrder++;
            }
        }
        if (nextOrder != levelState.entityRenderStates.size()) {
            this.gaps.add("entity render_order count mismatch: matched=" + nextOrder + " extracted=" + levelState.entityRenderStates.size());
        }

        allEntities.sort(Comparator.comparingInt(Entity::getId));
        JsonArray entitiesArr = new JsonArray();
        for (Entity entity : allEntities) {
            float entityPartialTicks = deltaTracker.getGameTimeDeltaPartialTick(!tickRateManager.isEntityFrozen(entity));
            boolean shouldRender = dispatcher.shouldRender(entity, cullFrustum, camX, camY, camZ, entityPartialTicks);
            Integer renderOrder = renderOrderById.get(entity.getId());
            EntityRenderState extractedState = renderStateById.get(entity.getId());

            JsonObject eObj = new JsonObject();
            eObj.addProperty("id", entity.getId());
            eObj.addProperty("type", BuiltInRegistries.ENTITY_TYPE.getKey(entity.getType()).toString());
            eObj.add("pos", doubleArray(entity.getX(), entity.getY(), entity.getZ()));
            eObj.add("pos_bits", doubleBitsArray(entity.getX(), entity.getY(), entity.getZ()));
            eObj.addProperty("yRot_bits", fbits(entity.getYRot()));
            eObj.addProperty("xRot_bits", fbits(entity.getXRot()));
            eObj.addProperty("eye_height_bits", fbits(entity.getEyeHeight()));
            eObj.addProperty("should_render", shouldRender);
            if (renderOrder == null) {
                eObj.add("render_order", JsonNull.INSTANCE);
            } else {
                eObj.addProperty("render_order", renderOrder);
            }
            if (entity instanceof Display display) {
                eObj.add("display", extractDisplayJson(mc, dispatcher, display, extractedState, camState, camX, camY, camZ, entityPartialTicks));
            } else {
                eObj.add("display", JsonNull.INSTANCE);
            }
            entitiesArr.add(eObj);
        }
        root.add("entities", entitiesArr);
        root.addProperty("screenshot", this.screenshotDir.resolve("settled.png").toAbsolutePath().toString());
        return root;
    }

    private JsonObject extractDisplayJson(
        Minecraft mc,
        EntityRenderDispatcher dispatcher,
        Display display,
        @Nullable EntityRenderState extractedState,
        CameraRenderState camState,
        double camX,
        double camY,
        double camZ,
        float partialTicks
    ) throws ReflectiveOperationException {
        DisplayEntityRenderState displayState;
        if (extractedState instanceof DisplayEntityRenderState ders) {
            displayState = ders;
        } else {
            displayState = (DisplayEntityRenderState) dispatcher.extractEntity(display, partialTicks);
        }

        Display.RenderState rs = displayState.renderState;
        float progress = displayState.interpolationProgress;
        JsonObject dObj = new JsonObject();

        String kind;
        if (display instanceof Display.TextDisplay) {
            kind = "text";
        } else if (display instanceof Display.BlockDisplay) {
            kind = "block";
        } else if (display instanceof Display.ItemDisplay) {
            kind = "item";
        } else {
            kind = "unknown";
        }
        dObj.addProperty("kind", kind);
        dObj.addProperty("billboard", rs == null ? "fixed" : rs.billboardConstraints().getSerializedName());
        dObj.addProperty("view_range_bits", fbits((Float) DISPLAY_GET_VIEW_RANGE.invoke(display)));
        dObj.addProperty("width_bits", fbits((Float) DISPLAY_GET_WIDTH.invoke(display)));
        dObj.addProperty("height_bits", fbits((Float) DISPLAY_GET_HEIGHT.invoke(display)));

        if (rs == null || rs.brightnessOverride() == Display.NO_BRIGHTNESS_OVERRIDE) {
            dObj.add("brightness_override", JsonNull.INSTANCE);
        } else {
            dObj.addProperty("brightness_override", rs.brightnessOverride());
        }
        dObj.addProperty("shadow_radius_bits", fbits(rs == null ? 0.0F : rs.shadowRadius().get(progress)));
        dObj.addProperty("shadow_strength_bits", fbits(rs == null ? 1.0F : rs.shadowStrength().get(progress)));
        dObj.addProperty("glow_color", rs == null ? -1 : rs.glowColorOverride());

        Transformation t = rs == null ? Transformation.IDENTITY : rs.transformation().get(progress);
        JsonObject tObj = new JsonObject();
        Vector3fc tr = t.translation();
        Quaternionfc lr = t.leftRotation();
        Vector3fc sc = t.scale();
        Quaternionfc rr = t.rightRotation();
        tObj.add("translation_bits", floatBitsArray(tr.x(), tr.y(), tr.z()));
        tObj.add("left_rotation_bits", floatBitsArray(lr.x(), lr.y(), lr.z(), lr.w()));
        tObj.add("scale_bits", floatBitsArray(sc.x(), sc.y(), sc.z()));
        tObj.add("right_rotation_bits", floatBitsArray(rr.x(), rr.y(), rr.z(), rr.w()));
        dObj.add("transformation", tObj);

        Matrix4f renderPose = null;
        Matrix4f displayPose = null;
        if (rs != null && displayState.hasSubState()) {
            PoseCaptureCollector collector = new PoseCaptureCollector();
            dispatcher.submit(
                displayState, camState,
                displayState.x - camX, displayState.y - camY, displayState.z - camZ,
                new PoseStack(), collector
            );
            renderPose = collector.capturedPose;

            EntityRenderer<?, ?> renderer = dispatcher.getRenderer(displayState);
            if (renderer instanceof DisplayRenderer<?, ?, ?> dr) {
                PoseStack baseStack = new PoseStack();
                Vec3 offset = dispatcher.getRenderer(displayState).getRenderOffset(displayState);
                baseStack.translate(
                    displayState.x - camX + offset.x(),
                    displayState.y - camY + offset.y(),
                    displayState.z - camZ + offset.z()
                );
                Quaternionf orient = (Quaternionf) DISPLAY_RENDERER_CALC_ORIENTATION.invoke(dr, rs, displayState, new Quaternionf());
                baseStack.rotate(orient);
                baseStack.mulPose(t);
                displayPose = new Matrix4f(baseStack.last().pose());
            }
        }
        dObj.add("render_pose_bits", renderPose == null ? JsonNull.INSTANCE : matrixBitsArray(renderPose));
        dObj.add("display_pose_bits", displayPose == null ? JsonNull.INSTANCE : matrixBitsArray(displayPose));

        if (display instanceof Display.TextDisplay textDisplay && displayState instanceof TextDisplayEntityRenderState textState) {
            Display.TextDisplay.TextRenderState trs = textState.textRenderState;
            if (trs != null) {
                JsonObject textObj = new JsonObject();
                JsonElement encoded = ComponentSerialization.CODEC
                    .encodeStart(mc.level.registryAccess().createSerializationContext(JsonOps.INSTANCE), trs.text())
                    .getOrThrow();
                textObj.addProperty("json", COMPACT_GSON.toJson(encoded));
                textObj.addProperty("line_width", trs.lineWidth());
                textObj.addProperty("background", trs.backgroundColor().get(progress));
                textObj.addProperty("text_opacity", trs.textOpacity().get(progress));
                textObj.addProperty("flags", (int) trs.flags());
                int lineCount = textState.cachedInfo == null ? 0 : textState.cachedInfo.lines().size();
                textObj.addProperty("lines", lineCount);
                dObj.add("text", textObj);
            } else {
                dObj.add("text", JsonNull.INSTANCE);
            }
        } else {
            dObj.add("text", JsonNull.INSTANCE);
        }

        if (display instanceof Display.BlockDisplay blockDisplay) {
            Display.BlockDisplay.BlockRenderState brs = blockDisplay.blockRenderState();
            addNullableString(dObj, "block_state", brs == null ? null : BlockStateParser.serialize(brs.blockState()));
        } else {
            dObj.add("block_state", JsonNull.INSTANCE);
        }

        if (display instanceof Display.ItemDisplay itemDisplay) {
            Display.ItemDisplay.ItemRenderState irs = itemDisplay.itemRenderState();
            if (irs != null) {
                dObj.addProperty("item", BuiltInRegistries.ITEM.getKey(irs.itemStack().getItem()).toString());
                dObj.addProperty("item_display", irs.itemTransform().getSerializedName());
            } else {
                dObj.add("item", JsonNull.INSTANCE);
                dObj.add("item_display", JsonNull.INSTANCE);
            }
        } else {
            dObj.add("item", JsonNull.INSTANCE);
            dObj.add("item_display", JsonNull.INSTANCE);
        }

        return dObj;
    }

    private static @Nullable String playerInfoGameMode(Minecraft mc, LocalPlayer player) {
        ClientPacketListener conn = mc.getConnection();
        if (conn == null) {
            return null;
        }
        PlayerInfo info = conn.getPlayerInfo(player.getUUID());
        return info == null ? null : info.getGameMode().getSerializedName();
    }

    private static void addNullableString(JsonObject obj, String key, @Nullable String value) {
        if (value == null) {
            obj.add(key, JsonNull.INSTANCE);
        } else {
            obj.addProperty(key, value);
        }
    }

    private static String fbits(float v) {
        return String.format(Locale.ROOT, "%08x", Float.floatToRawIntBits(v));
    }

    private static String dbits(double v) {
        return String.format(Locale.ROOT, "%016x", Double.doubleToRawLongBits(v));
    }

    private static JsonArray doubleArray(double a, double b, double c) {
        JsonArray arr = new JsonArray();
        arr.add(a);
        arr.add(b);
        arr.add(c);
        return arr;
    }

    private static JsonArray doubleBitsArray(double a, double b, double c) {
        JsonArray arr = new JsonArray();
        arr.add(dbits(a));
        arr.add(dbits(b));
        arr.add(dbits(c));
        return arr;
    }

    private static JsonArray floatBitsArray(float... values) {
        JsonArray arr = new JsonArray();
        for (float v : values) {
            arr.add(fbits(v));
        }
        return arr;
    }

    private static JsonArray matrixBitsArray(Matrix4f mat) {
        float[] buf = new float[16];
        mat.get(buf);
        return floatBitsArray(buf);
    }

    private final class ProbeMetricsRecorder implements MetricsRecorder {
        private final Minecraft mc;
        private final ProbeProfiler profiler;
        private boolean inTick;

        ProbeMetricsRecorder(Minecraft mc) {
            this.mc = mc;
            this.profiler = new ProbeProfiler(mc);
        }

        @Override
        public void end() {
        }

        @Override
        public void cancel() {
        }

        @Override
        public void startTick() {
            this.inTick = true;
            this.profiler.resetFrame();
        }

        @Override
        public void sampleDuringExtract() {
            GenpvpMenuProbe.this.onSampleDuringExtract(this.mc);
        }

        @Override
        public boolean isRecording() {
            // Return true only when constructProfiler() checks before startTick(), avoiding TimerQuery and cancel hooks inside runTick().
            return !this.inTick;
        }

        @Override
        public ProfilerFiller getProfiler() {
            return this.profiler;
        }

        @Override
        public void endTick() {
            this.inTick = false;
            GenpvpMenuProbe.this.onFrameEnd(this.mc);
        }
    }

    private final class ProbeProfiler implements ProfilerFiller {
        private final Minecraft mc;
        private final Deque<String> stack = new ArrayDeque<>();
        private boolean inClientTick;
        private int tickStartDepth;

        ProbeProfiler(Minecraft mc) {
            this.mc = mc;
        }

        void resetFrame() {
            this.stack.clear();
            this.inClientTick = false;
        }

        @Override
        public void startTick() {
        }

        @Override
        public void endTick() {
        }

        @Override
        public void push(String name) {
            this.stack.push(name);
        }

        @Override
        public void push(Supplier<String> name) {
            this.push(name.get());
        }

        @Override
        public void pop() {
            String popped = this.stack.isEmpty() ? null : this.stack.pop();
            if (this.inClientTick && "keyboard".equals(popped) && this.stack.size() == this.tickStartDepth) {
                this.inClientTick = false;
                GenpvpMenuProbe.this.onClientTickEnd(this.mc);
            }
        }

        @Override
        public void popPush(String name) {
            this.pop();
            this.push(name);
        }

        @Override
        public void popPush(Supplier<String> name) {
            this.pop();
            this.push(name.get());
        }

        @Override
        public void markForCharting(MetricCategory category) {
        }

        @Override
        public void incrementCounter(String name, int amount) {
            if ("clientTick".equals(name)) {
                this.inClientTick = true;
                this.tickStartDepth = this.stack.size();
            }
        }

        @Override
        public void incrementCounter(Supplier<String> name, int amount) {
            this.incrementCounter(name.get(), amount);
        }
    }

    private final class CountingQueue<E> extends ConcurrentLinkedQueue<E> {
        @Override
        public @Nullable E poll() {
            E item = super.poll();
            if (item != null) {
                try {
                    if (LISTENER_FIELD.get(item) instanceof ClientPacketListener) {
                        GenpvpMenuProbe.this.pendingPacketsHandled.incrementAndGet();
                    }
                } catch (IllegalAccessException ignored) {
                }
            }
            return item;
        }
    }

    private static final class PoseCaptureCollector implements SubmitNodeCollector {
        @Nullable Matrix4f capturedPose;

        private void capture(PoseStack poseStack) {
            if (this.capturedPose == null) {
                this.capturedPose = new Matrix4f(poseStack.last().pose());
            }
        }

        @Override
        public OrderedSubmitNodeCollector order(int order) {
            return this;
        }

        @Override
        public void submitShadow(PoseStack poseStack, float radius, List<EntityRenderState.ShadowPiece> pieces) {
        }

        @Override
        public void submitNameTag(PoseStack poseStack, @Nullable Vec3 nameTagAttachment, int offset, Component name, boolean seeThrough, int lightCoords, CameraRenderState camera) {
        }

        @Override
        public void submitText(PoseStack poseStack, float x, float y, FormattedCharSequence string, boolean dropShadow, Font.DisplayMode displayMode, int lightCoords, int color, int backgroundColor, int outlineColor) {
            capture(poseStack);
        }

        @Override
        public void submitTextBackground(PoseStack poseStack, float x0, float y0, float x1, float y1, int color, Font.DisplayMode displayMode, int lightCoords) {
            capture(poseStack);
        }

        @Override
        public void submitFlame(PoseStack poseStack, EntityRenderState renderState, Quaternionf rotation) {
        }

        @Override
        public void submitLeash(PoseStack poseStack, EntityRenderState.LeashState leashState) {
        }

        @Override
        public <S> void submitModel(Model<? super S> model, S state, PoseStack poseStack, RenderType renderType, int lightCoords, int overlayCoords, int tintedColor, @Nullable UvMapping uvMapping, int outlineColor) {
            capture(poseStack);
        }

        @Override
        public <S> void submitCrumblingOverlay(Model<? super S> model, S state, PoseStack poseStack, RenderType renderType, int lightCoords, int overlayCoords, int tintedColor, ModelFeatureRenderer.CrumblingOverlay crumblingOverlay) {
        }

        @Override
        public void submitMovingBlock(PoseStack poseStack, MovingBlockRenderState movingBlockRenderState, int outlineColor) {
            capture(poseStack);
        }

        @Override
        public void submitBlockModel(PoseStack poseStack, RenderType renderType, List<BlockStateModelPart> parts, int[] tintLayers, int lightCoords, int overlayCoords, int outlineColor) {
            capture(poseStack);
        }

        @Override
        public void submitBreakingBlockModel(PoseStack poseStack, List<BlockStateModelPart> parts, int progress, boolean isBlockTranslucent) {
        }

        @Override
        public void submitShapeOutline(PoseStack poseStack, VoxelShape shape, RenderType renderType, int color, float width, boolean afterTerrain) {
        }

        @Override
        public void submitItem(PoseStack poseStack, ItemDisplayContext displayContext, int lightCoords, int overlayCoords, int outlineColor, int[] tintLayers, ItemQuads quads, ItemStackRenderState.FoilType foilType) {
            capture(poseStack);
        }

        @Override
        public void submitCustomGeometry(PoseStack poseStack, RenderType renderType, SubmitNodeCollector.CustomGeometryRenderer customGeometryRenderer) {
            capture(poseStack);
        }

        @Override
        public void submitQuadParticleGroup(QuadParticleRenderState particles) {
        }

        @Override
        public void submitGizmoPrimitives(DrawableGizmoPrimitives.Group group, CameraRenderState camera, boolean onTop) {
        }
    }
}
