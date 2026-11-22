package dev.sunriselab.parity;

import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.mojang.authlib.GameProfile;
import io.netty.buffer.ByteBuf;
import io.netty.buffer.Unpooled;
import io.netty.channel.ChannelFutureListener;
import io.netty.channel.embedded.EmbeddedChannel;
import java.io.File;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HexFormat;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.OptionalInt;
import java.util.Queue;
import java.util.Set;
import java.util.UUID;
import net.minecraft.client.Camera;
import net.minecraft.client.Minecraft;
import net.minecraft.client.Options;
import net.minecraft.client.gui.Gui;
import net.minecraft.client.gui.Hud;
import net.minecraft.client.gui.components.ChatComponent;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.multiplayer.ClientCommonPacketListenerImpl;
import net.minecraft.client.multiplayer.ClientConfigurationPacketListenerImpl;
import net.minecraft.client.multiplayer.ClientDebugSubscriber;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.client.multiplayer.CommonListenerCookie;
import net.minecraft.client.multiplayer.LevelLoadTracker;
import net.minecraft.client.multiplayer.MultiPlayerGameMode;
import net.minecraft.client.multiplayer.chat.ChatAbilities;
import net.minecraft.client.player.ItemActivation;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.client.quickplay.QuickPlayLog;
import net.minecraft.client.renderer.GameRenderer;
import net.minecraft.client.renderer.debug.DebugRenderer;
import net.minecraft.client.renderer.extract.LevelExtractor;
import net.minecraft.client.sounds.MusicManager;
import net.minecraft.client.sounds.SoundManager;
import net.minecraft.client.telemetry.WorldSessionTelemetryManager;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.core.Holder;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.Registries;
import net.minecraft.network.Connection;
import net.minecraft.network.DisconnectionDetails;
import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.network.PacketBundlePacker;
import net.minecraft.network.PacketDecoder;
import net.minecraft.network.PacketProcessor;
import net.minecraft.network.ProtocolInfo;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.contents.TranslatableContents;
import net.minecraft.network.codec.StreamCodec;
import net.minecraft.network.protocol.Packet;
import net.minecraft.network.protocol.PacketFlow;
import net.minecraft.network.protocol.common.ClientboundKeepAlivePacket;
import net.minecraft.network.protocol.common.ClientboundPingPacket;
import net.minecraft.network.protocol.common.ClientboundPostEffectsPacket;
import net.minecraft.network.protocol.common.ServerboundKeepAlivePacket;
import net.minecraft.network.protocol.common.ServerboundPongPacket;
import net.minecraft.network.protocol.configuration.ConfigurationProtocols;
import net.minecraft.network.protocol.game.ClientGamePacketListener;
import net.minecraft.network.protocol.game.ClientboundAddEntityPacket;
import net.minecraft.network.protocol.game.ClientboundBundleDelimiterPacket;
import net.minecraft.network.protocol.game.ClientboundBundlePacket;
import net.minecraft.network.protocol.game.ClientboundLoginPacket;
import net.minecraft.network.protocol.game.ClientboundPlayerRotationPacket;
import net.minecraft.network.protocol.game.ClientboundRespawnPacket;
import net.minecraft.network.protocol.game.ClientboundSetCameraPacket;
import net.minecraft.network.protocol.game.ClientboundSetEntityDataPacket;
import net.minecraft.network.protocol.game.ClientboundStartConfigurationPacket;
import net.minecraft.network.protocol.game.CommonPlayerSpawnInfo;
import net.minecraft.network.protocol.game.GameProtocols;
import net.minecraft.network.protocol.game.ServerboundMovePlayerPacket;
import net.minecraft.network.syncher.EntityDataSerializers;
import net.minecraft.network.syncher.SynchedEntityData;
import net.minecraft.resources.Identifier;
import net.minecraft.server.ServerLinks;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.EntityTypes;
import net.minecraft.world.entity.Interaction;
import net.minecraft.world.entity.decoration.painting.PaintingVariant;
import net.minecraft.world.flag.FeatureFlags;
import net.minecraft.world.level.GameType;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.dimension.BuiltinDimensionTypes;
import net.minecraft.world.phys.Vec3;

/**
 * Records 26.3 network parity witnesses from real vanilla classes:
 *   1. bundle.json: PacketBundlePacker, BundlerInfo, PacketProcessor, ClientPacketListener.handleBundlePacket
 *   2. post_effects.json: ConfigurationProtocols/GameProtocols codecs, ClientConfigurationPacketListenerImpl, ClientPacketListener login/respawn/post_effects
 *   3. entity_data.json: EntityDataSerializers, ClientboundSetEntityDataPacket.STREAM_CODEC, PacketDecoder + Connection.exceptionCaught
 */
public final class VanillaNetworkParityOracle {
    private static final HexFormat HEX = HexFormat.of();
    private static Object unsafe;
    private static Method allocate;

    static <T> T alloc(Class<T> type) throws Exception {
        return type.cast(allocate.invoke(unsafe, type));
    }

    static Field findField(Class<?> owner, String name) throws Exception {
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

    static void put(Object obj, Class<?> owner, String name, Object val) throws Exception {
        findField(owner, name).set(obj, val);
    }

    static Object get(Object obj, Class<?> owner, String name) throws Exception {
        return findField(owner, name).get(obj);
    }

    static String f32Bits(float v) {
        return String.format("%08x", Float.floatToRawIntBits(v));
    }

    static String disconnectKey(Component c) {
        return c.getContents() instanceof TranslatableContents t ? t.getKey() : c.getString();
    }

    static final class RecordingConnection extends Connection {
        final List<String> sent = new ArrayList<>();
        String disconnectKey = "none";
        String disconnectText = "";

        RecordingConnection() {
            super(PacketFlow.CLIENTBOUND);
        }

        @Override
        public void send(Packet<?> packet, ChannelFutureListener listener, boolean flush) {
            record(packet);
        }

        @Override
        public void send(Packet<?> packet, ChannelFutureListener listener) {
            record(packet);
        }

        @Override
        public void send(Packet<?> packet) {
            record(packet);
        }

        private void record(Packet<?> packet) {
            if (packet instanceof ServerboundPongPacket p) {
                sent.add("pong:" + p.getId());
            } else if (packet instanceof ServerboundKeepAlivePacket k) {
                sent.add("keep_alive:" + k.getId());
            } else if (packet instanceof ServerboundMovePlayerPacket.Rot r) {
                sent.add("move_player_rot:" + f32Bits(r.getYRot(0f)) + ":" + f32Bits(r.getXRot(0f)));
            } else {
                sent.add(packet.type().id().toString());
            }
        }

        @Override
        public void disconnect(Component reason) {
            this.disconnectKey = VanillaNetworkParityOracle.disconnectKey(reason);
            this.disconnectText = reason.getString();
        }

        @Override
        public void disconnect(DisconnectionDetails details) {
            disconnect(details.reason());
        }

        @Override
        public boolean isConnected() {
            return true;
        }
    }

    static final class StubWorld extends ClientLevel {
        Map<Integer, Entity> entities;

        StubWorld() {
            super(null, null, null, null, 0, 0, null, false, 0L, 0);
        }

        @Override
        public boolean isClientSide() {
            return true;
        }

        @Override
        public RegistryAccess registryAccess() {
            return ParityData.access;
        }

        @Override
        public void addEntity(Entity entity) {
            entities.put(entity.getId(), entity);
        }

        @Override
        public Entity getEntity(int id) {
            return entities.get(id);
        }
    }

    static final class StubChat extends ChatComponent {
        StubChat() {
            super(null);
        }

        @Override
        public void preserveCurrentChatScreen() {
        }
    }

    static final class StubHud extends Hud {
        ChatComponent chat;

        StubHud() {
            super(null);
        }

        @Override
        public ChatComponent getChat() {
            return chat;
        }
    }

    static final class StubGui extends Gui {
        StubGui() {
            super(null, null, null);
        }

        @Override
        public Screen screen() {
            return null;
        }

        @Override
        public void setScreen(Screen screen) {
        }
    }

    static final class StubOptions extends Options {
        StubOptions() {
            super(null, null);
        }

        @Override
        public void setServerRenderDistance(int distance) {
        }
    }

    static final class StubDebugRenderer extends DebugRenderer {
        StubDebugRenderer() {
            super();
        }

        @Override
        public void refreshRendererList() {
        }
    }

    static final class StubDebugEntries extends net.minecraft.client.gui.components.debug.DebugScreenEntryList {
        StubDebugEntries() {
            super(null, null);
        }

        @Override
        public void rebuildCurrentList() {
        }
    }

    static final class StubMusicManager extends MusicManager {
        StubMusicManager() {
            super(null);
        }

        @Override
        public void stopPlaying() {
        }
    }

    static final class StubTelemetry extends WorldSessionTelemetryManager {
        StubTelemetry() {
            super(null, false, null, null, null);
        }

        @Override
        public void onPlayerInfoReceived(GameType type, boolean hardcore) {
        }
    }

    static final class StubMinecraft extends Minecraft {
        MusicManager stubMusic;
        SoundManager stubSound;

        StubMinecraft() {
            super(null);
        }

        @Override
        public void setLevel(ClientLevel level) {
            this.level = level;
        }

        @Override
        public void setScreenAndShow(Screen screen) {
        }

        @Override
        public void setCameraEntity(Entity entity) {
            this.gameRenderer.mainCamera().setEntity(entity);
        }

        @Override
        public Entity getCameraEntity() {
            return this.gameRenderer.mainCamera().entity();
        }

        @Override
        public ChatAbilities computeChatAbilities() {
            return ChatAbilities.NO_RESTRICTIONS;
        }

        @Override
        public MusicManager getMusicManager() {
            return stubMusic;
        }

        @Override
        public SoundManager getSoundManager() {
            return stubSound;
        }

        @Override
        public QuickPlayLog quickPlayLog() {
            return QuickPlayLog.of(null);
        }

        @Override
        public net.minecraft.client.server.IntegratedServer getSingleplayerServer() {
            return null;
        }
    }

    static StubMinecraft makeMinecraft(File gameDir) throws Exception {
        StubMinecraft mc = alloc(StubMinecraft.class);
        put(mc, Minecraft.class, "packetProcessor", new PacketProcessor(Thread.currentThread()));
        put(mc, Minecraft.class, "gameDirectory", gameDir);
        StubGui gui = alloc(StubGui.class);
        StubHud hud = alloc(StubHud.class);
        hud.chat = alloc(StubChat.class);
        put(gui, Gui.class, "hud", hud);
        put(mc, Minecraft.class, "gui", gui);
        put(mc, Minecraft.class, "options", alloc(StubOptions.class));
        put(mc, Minecraft.class, "debugEntries", alloc(StubDebugEntries.class));
        LevelExtractor extractor = alloc(LevelExtractor.class);
        put(extractor, LevelExtractor.class, "debugRenderer", alloc(StubDebugRenderer.class));
        put(mc, Minecraft.class, "levelExtractor", extractor);
        Camera cam = alloc(Camera.class);
        GameRenderer gr = alloc(GameRenderer.class);
        put(gr, GameRenderer.class, "mainCamera", cam);
        put(mc, Minecraft.class, "gameRenderer", gr);
        mc.stubMusic = alloc(StubMusicManager.class);
        mc.stubSound = alloc(SoundManager.class);
        put(null, Minecraft.class, "instance", mc);
        return mc;
    }

    static ClientPacketListener makePlayListener(StubMinecraft mc, RecordingConnection conn, ClientLevel level) throws Exception {
        ClientPacketListener listener = alloc(ClientPacketListener.class);
        put(listener, ClientCommonPacketListenerImpl.class, "minecraft", mc);
        put(listener, ClientCommonPacketListenerImpl.class, "connection", conn);
        put(listener, ClientCommonPacketListenerImpl.class, "serverLinks", ServerLinks.EMPTY);
        put(listener, ClientCommonPacketListenerImpl.class, "customReportDetails", Map.of());
        put(listener, ClientCommonPacketListenerImpl.class, "telemetryManager", alloc(StubTelemetry.class));
        put(listener, ClientCommonPacketListenerImpl.class, "seenPlayers", new HashMap<>());
        put(listener, ClientCommonPacketListenerImpl.class, "deferredPackets", new ArrayList<>());
        put(listener, ClientPacketListener.class, "localGameProfile", new GameProfile(new UUID(0, 1), "Oracle"));
        put(listener, ClientPacketListener.class, "registryAccess", ParityData.access);
        put(listener, ClientPacketListener.class, "enabledFeatures", FeatureFlags.DEFAULT_FLAGS);
        put(listener, ClientPacketListener.class, "level", level);
        put(listener, ClientPacketListener.class, "removedPlayerVehicleId", OptionalInt.empty());
        put(listener, ClientPacketListener.class, "clockManager", new net.minecraft.client.ClientClockManager());
        put(listener, ClientPacketListener.class, "playerInfoMap", new HashMap<>());
        put(listener, ClientPacketListener.class, "listedPlayers", new it.unimi.dsi.fastutil.objects.ReferenceOpenHashSet<>());
        ClientDebugSubscriber dbg = alloc(ClientDebugSubscriber.class);
        put(dbg, ClientDebugSubscriber.class, "valuesBySubscription", new HashMap<>());
        put(dbg, ClientDebugSubscriber.class, "remoteSubscriptions", Set.of());
        put(listener, ClientPacketListener.class, "debugSubscriber", dbg);
        return listener;
    }

    static StubWorld makeStubWorld() throws Exception {
        StubWorld world = alloc(StubWorld.class);
        world.entities = new HashMap<>();
        put(world, Level.class, "registryAccess", ParityData.access);
        put(world, Level.class, "isClientSide", true);
        return world;
    }

    static LocalPlayer makeRealPlayer(StubMinecraft mc, ClientLevel level, ClientPacketListener listener, int id) {
        LocalPlayer p = new LocalPlayer(
            mc,
            level,
            listener,
            new net.minecraft.stats.StatsCounter(),
            new net.minecraft.client.ClientRecipeBook(),
            net.minecraft.world.entity.player.Input.EMPTY,
            false,
            ChatAbilities.NO_RESTRICTIONS,
            new ItemActivation()
        );
        p.setId(id);
        return p;
    }

    @SuppressWarnings("unchecked")
    static ByteBuf encodeFrame(ProtocolInfo<?> protocol, Packet<?> packet) {
        var codec = (StreamCodec<ByteBuf, Packet<?>>) (Object) protocol.codec();
        ByteBuf buf = Unpooled.buffer();
        codec.encode(buf, packet);
        return buf;
    }

    static String toHex(ByteBuf buf) {
        byte[] bytes = new byte[buf.readableBytes()];
        buf.getBytes(buf.readerIndex(), bytes);
        return HEX.formatHex(bytes);
    }

    static int queuedCount(PacketProcessor processor) throws Exception {
        return ((Queue<?>) get(processor, PacketProcessor.class, "packetsToBeHandled")).size();
    }

    static void deliverFromNetThread(Packet<?> packet, ClientCommonPacketListenerImpl listener) throws Exception {
        Thread net = new Thread(() -> {
            @SuppressWarnings("unchecked")
            Packet<ClientCommonPacketListenerImpl> p = (Packet<ClientCommonPacketListenerImpl>) packet;
            try {
                p.handle(listener);
            } catch (net.minecraft.server.RunningOnDifferentThreadException ignored) {
            }
        }, "oracle-net");
        net.start();
        net.join();
    }

    static JsonArray strArray(List<String> list) {
        JsonArray a = new JsonArray();
        for (String s : list) a.add(s);
        return a;
    }

    static JsonArray idList(List<Identifier> list) {
        JsonArray a = new JsonArray();
        for (Identifier id : list) a.add(id.toString());
        return a;
    }

    static Packet<?> delimiterOf(ProtocolInfo<ClientGamePacketListener> play) {
        List<Packet<?>> out = new ArrayList<>();
        play.bundlerInfo().unbundlePacket(new ClientboundBundlePacket(List.of()), out::add);
        return out.get(0);
    }

    static JsonObject recordBundles(ProtocolInfo<ClientGamePacketListener> play, File gameDir) throws Exception {
        Packet<?> delimiter = delimiterOf(play);
        JsonObject root = new JsonObject();
        root.addProperty("game_version", "26.3");
        root.addProperty("bundle_size_limit", net.minecraft.network.protocol.BundlerInfo.BUNDLE_SIZE_LIMIT);

        // 1. Step-by-step PacketBundlePacker on genpvp's [delimiter, ping(-26), player_rotation(0, false, 0, false), delimiter]
        {
            EmbeddedChannel ch = new EmbeddedChannel(new PacketDecoder<>(play), new PacketBundlePacker(play.bundlerInfo()));
            Packet<?>[] frames = {
                delimiter,
                new ClientboundPingPacket(-26),
                new ClientboundPlayerRotationPacket(0.0f, false, 0.0f, false),
                delimiter
            };
            JsonArray steps = new JsonArray();
            for (Packet<?> f : frames) {
                ch.writeInbound(encodeFrame(play, f));
                JsonObject s = new JsonObject();
                s.addProperty("sent", f.type().id().toString());
                s.addProperty("emitted_count", ch.inboundMessages().size());
                steps.add(s);
            }
            Packet<?> emitted = ch.readInbound();
            JsonObject obj = new JsonObject();
            obj.add("steps", steps);
            obj.addProperty("emitted_is_bundle", emitted instanceof ClientboundBundlePacket);
            JsonArray subs = new JsonArray();
            if (emitted instanceof ClientboundBundlePacket bp) {
                for (Packet<?> sp : bp.subPackets()) subs.add(sp.type().id().toString());
            }
            obj.add("sub_packets", subs);
            ch.finishAndReleaseAll();
            root.add("ping_rotation_wire_steps", obj);
        }

        // 2. Unclosed bundle emits nothing
        {
            EmbeddedChannel ch = new EmbeddedChannel(new PacketDecoder<>(play), new PacketBundlePacker(play.bundlerInfo()));
            ch.writeInbound(encodeFrame(play, delimiter));
            ch.writeInbound(encodeFrame(play, new ClientboundPingPacket(42)));
            ch.writeInbound(encodeFrame(play, new ClientboundPlayerRotationPacket(15.0f, true, -5.0f, true)));
            JsonObject obj = new JsonObject();
            obj.addProperty("emitted_count", ch.inboundMessages().size());
            ch.finishAndReleaseAll();
            root.add("unclosed_bundle", obj);
        }

        // 3. Empty bundle [delimiter, delimiter]
        {
            EmbeddedChannel ch = new EmbeddedChannel(new PacketDecoder<>(play), new PacketBundlePacker(play.bundlerInfo()));
            ch.writeInbound(encodeFrame(play, delimiter));
            ch.writeInbound(encodeFrame(play, delimiter));
            Packet<?> emitted = ch.readInbound();
            int subCount = 0;
            if (emitted instanceof ClientboundBundlePacket bp) {
                for (Packet<?> ignored : bp.subPackets()) subCount++;
            }
            JsonObject obj = new JsonObject();
            obj.addProperty("emitted_is_bundle", emitted instanceof ClientboundBundlePacket);
            obj.addProperty("sub_count", subCount);
            ch.finishAndReleaseAll();
            root.add("empty_bundle", obj);
        }

        // 4. Back-to-back bundles followed by standalone packet
        {
            EmbeddedChannel ch = new EmbeddedChannel(new PacketDecoder<>(play), new PacketBundlePacker(play.bundlerInfo()));
            Packet<?>[] frames = {
                delimiter,
                new ClientboundPingPacket(1),
                delimiter,
                delimiter,
                new ClientboundPingPacket(2),
                delimiter,
                new ClientboundPingPacket(3)
            };
            for (Packet<?> f : frames) ch.writeInbound(encodeFrame(play, f));
            JsonArray emitted = new JsonArray();
            Packet<?> p;
            while ((p = ch.readInbound()) != null) {
                JsonObject row = new JsonObject();
                if (p instanceof ClientboundBundlePacket bp) {
                    row.addProperty("kind", "bundle");
                    JsonArray ids = new JsonArray();
                    for (Packet<?> sp : bp.subPackets()) ids.add(((ClientboundPingPacket) sp).getId());
                    row.add("ping_ids", ids);
                } else {
                    row.addProperty("kind", "single");
                    row.addProperty("ping_id", ((ClientboundPingPacket) p).getId());
                }
                emitted.add(row);
            }
            ch.finishAndReleaseAll();
            root.add("back_to_back_and_single", emitted);
        }

        // 5. Bundle size limit: 4096 passes, 4097 fails and disconnects via Connection.exceptionCaught
        {
            JsonObject obj = new JsonObject();
            EmbeddedChannel okCh = new EmbeddedChannel(new PacketDecoder<>(play), new PacketBundlePacker(play.bundlerInfo()));
            okCh.writeInbound(encodeFrame(play, delimiter));
            for (int i = 0; i < 4096; i++) okCh.writeInbound(encodeFrame(play, new ClientboundPingPacket(i)));
            okCh.writeInbound(encodeFrame(play, delimiter));
            ClientboundBundlePacket bp = okCh.readInbound();
            int count = 0;
            for (Packet<?> ignored : bp.subPackets()) count++;
            obj.addProperty("limit_4096_ok", true);
            obj.addProperty("limit_4096_sub_count", count);
            okCh.finishAndReleaseAll();

            RecordingConnection conn = new RecordingConnection();
            EmbeddedChannel failCh = new EmbeddedChannel(new PacketDecoder<>(play), new PacketBundlePacker(play.bundlerInfo()), conn);
            failCh.writeInbound(encodeFrame(play, delimiter));
            for (int i = 0; i < 4096; i++) failCh.writeInbound(encodeFrame(play, new ClientboundPingPacket(i)));
            failCh.writeInbound(encodeFrame(play, new ClientboundPingPacket(4096)));
            obj.addProperty("limit_4097_disconnect_key", conn.disconnectKey);
            obj.addProperty("limit_4097_disconnect_text", conn.disconnectText);
            failCh.finishAndReleaseAll();
            root.add("size_limit", obj);
        }

        // 6. Terminal packet (StartConfiguration) inside vs outside bundle
        {
            JsonObject obj = new JsonObject();
            RecordingConnection conn = new RecordingConnection();
            EmbeddedChannel failCh = new EmbeddedChannel(new PacketDecoder<>(play), new PacketBundlePacker(play.bundlerInfo()), conn);
            failCh.writeInbound(encodeFrame(play, delimiter));
            failCh.writeInbound(encodeFrame(play, ClientboundStartConfigurationPacket.INSTANCE));
            obj.addProperty("inside_disconnect_key", conn.disconnectKey);
            obj.addProperty("inside_disconnect_text", conn.disconnectText);
            failCh.finishAndReleaseAll();

            EmbeddedChannel okCh = new EmbeddedChannel(new PacketDecoder<>(play), new PacketBundlePacker(play.bundlerInfo()));
            okCh.writeInbound(encodeFrame(play, ClientboundStartConfigurationPacket.INSTANCE));
            Packet<?> out = okCh.readInbound();
            obj.addProperty("outside_emitted_type", out.type().id().toString());
            obj.addProperty("outside_is_terminal", out.isTerminal());
            obj.addProperty("outside_bundler_removed", okCh.pipeline().get(PacketBundlePacker.class) == null);
            okCh.finishAndReleaseAll();
            root.add("terminal_packet", obj);
        }

        // 7. Real ClientPacketListener + PacketProcessor: genpvp [ping(-26), player_rotation(22.5, false, -11.25, false)]
        {
            StubMinecraft mc = makeMinecraft(gameDir);
            StubWorld world = makeStubWorld();
            RecordingConnection conn = new RecordingConnection();
            ClientPacketListener listener = makePlayListener(mc, conn, world);
            LocalPlayer player = makeRealPlayer(mc, world, listener, 83081);
            mc.player = player;
            mc.setCameraEntity(player);

            EmbeddedChannel ch = new EmbeddedChannel(new PacketDecoder<>(play), new PacketBundlePacker(play.bundlerInfo()));
            ch.writeInbound(encodeFrame(play, delimiter));
            ch.writeInbound(encodeFrame(play, new ClientboundPingPacket(-26)));
            ch.writeInbound(encodeFrame(play, new ClientboundPlayerRotationPacket(22.5f, false, -11.25f, false)));
            ch.writeInbound(encodeFrame(play, delimiter));
            Packet<?> bundle = ch.readInbound();
            ch.finishAndReleaseAll();

            deliverFromNetThread(bundle, listener);
            JsonObject obj = new JsonObject();
            obj.addProperty("queued_tasks_before_process", queuedCount(mc.packetProcessor()));
            obj.add("sent_before_process", strArray(conn.sent));
            mc.packetProcessor().processQueuedPackets();
            obj.addProperty("queued_tasks_after_process", queuedCount(mc.packetProcessor()));
            obj.add("sent_after_process", strArray(conn.sent));
            obj.addProperty("player_yRot_bits", f32Bits(player.getYRot()));
            obj.addProperty("player_xRot_bits", f32Bits(player.getXRot()));
            root.add("genpvp_ping_rotation_dispatch", obj);
        }

        // 8. KeepAlive ordering: unbundled (answers on net thread) vs bundled (waits and stays in bundle order!)
        {
            JsonObject obj = new JsonObject();
            // Unbundled
            {
                StubMinecraft mc = makeMinecraft(gameDir);
                StubWorld world = makeStubWorld();
                RecordingConnection conn = new RecordingConnection();
                ClientPacketListener listener = makePlayListener(mc, conn, world);
                mc.player = makeRealPlayer(mc, world, listener, 1);

                deliverFromNetThread(new ClientboundPingPacket(7), listener);
                deliverFromNetThread(new ClientboundKeepAlivePacket(999L), listener);
                JsonObject u = new JsonObject();
                u.addProperty("queued_tasks_before_process", queuedCount(mc.packetProcessor()));
                u.add("sent_before_process", strArray(conn.sent));
                mc.packetProcessor().processQueuedPackets();
                u.add("sent_after_process", strArray(conn.sent));
                obj.add("unbundled", u);
            }
            // Bundled
            {
                StubMinecraft mc = makeMinecraft(gameDir);
                StubWorld world = makeStubWorld();
                RecordingConnection conn = new RecordingConnection();
                ClientPacketListener listener = makePlayListener(mc, conn, world);
                mc.player = makeRealPlayer(mc, world, listener, 1);

                EmbeddedChannel ch = new EmbeddedChannel(new PacketDecoder<>(play), new PacketBundlePacker(play.bundlerInfo()));
                ch.writeInbound(encodeFrame(play, delimiter));
                ch.writeInbound(encodeFrame(play, new ClientboundPingPacket(7)));
                ch.writeInbound(encodeFrame(play, new ClientboundKeepAlivePacket(999L)));
                ch.writeInbound(encodeFrame(play, new ClientboundPlayerRotationPacket(90.0f, false, 12.5f, false)));
                ch.writeInbound(encodeFrame(play, delimiter));
                Packet<?> bundle = ch.readInbound();
                ch.finishAndReleaseAll();

                deliverFromNetThread(bundle, listener);
                JsonObject b = new JsonObject();
                b.addProperty("queued_tasks_before_process", queuedCount(mc.packetProcessor()));
                b.add("sent_before_process", strArray(conn.sent));
                mc.packetProcessor().processQueuedPackets();
                b.add("sent_after_process", strArray(conn.sent));
                obj.add("bundled", b);
            }
            root.add("keepalive_ordering", obj);
        }

        // 9. Dependent packets inside a bundle across a tick boundary:
        //    [Delimiter, AddEntity(83082, interaction)] -> tick/process -> [SetEntityData(83082, 0.1, 1.9059), SetCamera(83082), Delimiter] -> process
        {
            StubMinecraft mc = makeMinecraft(gameDir);
            StubWorld world = makeStubWorld();
            RecordingConnection conn = new RecordingConnection();
            ClientPacketListener listener = makePlayListener(mc, conn, world);
            LocalPlayer player = makeRealPlayer(mc, world, listener, 83081);
            mc.player = player;
            mc.gameRenderer.mainCamera().setEntity(player);
            mc.setCameraEntity(player);

            EmbeddedChannel ch = new EmbeddedChannel(new PacketDecoder<>(play), new PacketBundlePacker(play.bundlerInfo()));
            ch.writeInbound(encodeFrame(play, delimiter));
            ch.writeInbound(encodeFrame(play, new ClientboundAddEntityPacket(
                83082, new UUID(0, 83082), 0.5, 300.0, 0.5, 0f, 0f, EntityTypes.INTERACTION, 0, Vec3.ZERO, 0.0
            )));
            // Nothing emitted yet: simulate a main-thread tick running before the rest of the bundle arrives
            Packet<?> partial = ch.readInbound();
            mc.packetProcessor().processQueuedPackets();

            JsonObject obj = new JsonObject();
            obj.addProperty("partial_emitted", partial != null);
            obj.addProperty("entity_exists_before_close", world.getEntity(83082) != null);
            obj.addProperty("camera_entity_id_before_close", mc.gameRenderer.mainCamera().entity().getId());

            // Now send SetEntityData (width=0.1, height=1.9059) + SetCamera(83082) + closing Delimiter
            List<SynchedEntityData.DataValue<?>> values = List.of(
                new SynchedEntityData.DataValue<>(8, EntityDataSerializers.FLOAT, 0.1f),
                new SynchedEntityData.DataValue<>(9, EntityDataSerializers.FLOAT, 1.9059f)
            );
            ch.writeInbound(encodeFrame(play, new ClientboundSetEntityDataPacket(83082, values)));
            var setCam = alloc(ClientboundSetCameraPacket.class);
            put(setCam, ClientboundSetCameraPacket.class, "cameraId", 83082);
            ch.writeInbound(encodeFrame(play, setCam));
            ch.writeInbound(encodeFrame(play, delimiter));
            Packet<?> bundle = ch.readInbound();
            ch.finishAndReleaseAll();

            deliverFromNetThread(bundle, listener);
            obj.addProperty("queued_tasks_before_close_process", queuedCount(mc.packetProcessor()));
            mc.packetProcessor().processQueuedPackets();
            obj.addProperty("queued_tasks_after_close_process", queuedCount(mc.packetProcessor()));

            Entity spawned = world.getEntity(83082);
            obj.addProperty("entity_exists_after_close", spawned != null);
            obj.addProperty("entity_type", EntityType.getKey(spawned.getType()).toString());
            obj.addProperty("entity_width_bits", f32Bits(spawned.getBbWidth()));
            obj.addProperty("entity_height_bits", f32Bits(spawned.getBbHeight()));
            obj.addProperty("entity_eye_height_bits", f32Bits(spawned.getEyeHeight()));
            obj.addProperty("camera_entity_id_after_close", mc.gameRenderer.mainCamera().entity().getId());
            root.add("dependent_entity_bundle", obj);
        }

        return root;
    }

    static JsonObject recordPostEffects(ProtocolInfo<ClientGamePacketListener> play, File gameDir) throws Exception {
        JsonObject root = new JsonObject();
        root.addProperty("game_version", "26.3");

        // 1. Wire format in both Configuration and Play protocols
        {
            JsonObject wire = new JsonObject();
            var empty = new ClientboundPostEffectsPacket(List.of());
            var two = new ClientboundPostEffectsPacket(List.of(Identifier.parse("minecraft:invert"), Identifier.parse("pack:glow")));

            ByteBuf cEmpty = encodeFrame(ConfigurationProtocols.CLIENTBOUND, empty);
            ByteBuf cTwo = encodeFrame(ConfigurationProtocols.CLIENTBOUND, two);
            ByteBuf pEmpty = encodeFrame(play, empty);
            ByteBuf pTwo = encodeFrame(play, two);

            wire.addProperty("config_packet_id", new FriendlyByteBuf(cEmpty.duplicate()).readVarInt());
            wire.addProperty("play_packet_id", new FriendlyByteBuf(pEmpty.duplicate()).readVarInt());
            wire.addProperty("config_empty_hex", toHex(cEmpty));
            wire.addProperty("config_two_hex", toHex(cTwo));
            wire.addProperty("play_empty_hex", toHex(pEmpty));
            wire.addProperty("play_two_hex", toHex(pTwo));
            root.add("wire", wire);
        }

        // 2. Configuration state: player is null -> throws IllegalStateException -> disconnect.packetError
        {
            JsonObject cfgCases = new JsonObject();
            for (var entry : List.of(
                Map.entry("empty", List.<Identifier>of()),
                Map.entry("nonempty", List.of(Identifier.parse("minecraft:invert")))
            )) {
                StubMinecraft mc = makeMinecraft(gameDir);
                RecordingConnection conn = new RecordingConnection();
                var cookie = new CommonListenerCookie(
                    new LevelLoadTracker(), new GameProfile(new UUID(0, 1), "Oracle"), alloc(StubTelemetry.class),
                    (RegistryAccess.Frozen) ParityData.access, FeatureFlags.DEFAULT_FLAGS,
                    null, null, null, Map.of(), null, Map.of(), ServerLinks.EMPTY, Map.of(), false
                );
                var listener = new ClientConfigurationPacketListenerImpl(mc, conn, cookie);
                deliverFromNetThread(new ClientboundPostEffectsPacket(entry.getValue()), listener);
                mc.packetProcessor().processQueuedPackets();
                JsonObject r = new JsonObject();
                r.addProperty("disconnect_key", conn.disconnectKey);
                cfgCases.add(entry.getKey(), r);
            }
            root.add("configuration_state", cfgCases);
        }

        // 3. Play state before Login: player is null -> disconnect.packetError
        {
            JsonObject preLogin = new JsonObject();
            for (var entry : List.of(
                Map.entry("empty", List.<Identifier>of()),
                Map.entry("nonempty", List.of(Identifier.parse("minecraft:invert")))
            )) {
                StubMinecraft mc = makeMinecraft(gameDir);
                RecordingConnection conn = new RecordingConnection();
                ClientPacketListener listener = makePlayListener(mc, conn, null);
                deliverFromNetThread(new ClientboundPostEffectsPacket(entry.getValue()), listener);
                mc.packetProcessor().processQueuedPackets();
                JsonObject r = new JsonObject();
                r.addProperty("disconnect_key", conn.disconnectKey);
                preLogin.add(entry.getKey(), r);
            }
            root.add("play_before_login", preLogin);
        }

        // 4. Play lifecycle: handleLogin, handlePostEffects, handleRespawn (keep=0,1,2,3), reconfigure+login, repeat login
        {
            Holder<net.minecraft.world.level.dimension.DimensionType> overworldType =
                ParityData.access.lookupOrThrow(Registries.DIMENSION_TYPE).getOrThrow(BuiltinDimensionTypes.OVERWORLD);
            Holder<net.minecraft.world.level.dimension.DimensionType> netherType =
                ParityData.access.lookupOrThrow(Registries.DIMENSION_TYPE).getOrThrow(BuiltinDimensionTypes.NETHER);

            var overworldSpawn = new CommonPlayerSpawnInfo(
                overworldType, Level.OVERWORLD, 1L, GameType.SURVIVAL, Optional.empty(), false, true, Optional.empty(), 0, 63
            );
            var netherSpawn = new CommonPlayerSpawnInfo(
                netherType, Level.NETHER, 2L, GameType.SURVIVAL, Optional.empty(), false, false, Optional.empty(), 0, 31
            );
            var loginPacket = new ClientboundLoginPacket(
                7, false, Set.of(Level.OVERWORLD, Level.NETHER), 20, 8, 8, false, true, false, overworldSpawn, false, false
            );

            StubMinecraft mc = makeMinecraft(gameDir);
            RecordingConnection conn = new RecordingConnection();
            ClientPacketListener listener = makePlayListener(mc, conn, null);

            JsonObject life = new JsonObject();
            listener.handleLogin(loginPacket);
            life.add("after_initial_login", idList(mc.player.getActivePostEffects()));
            life.addProperty("disconnect_after_login", conn.disconnectKey);

            listener.handlePostEffects(new ClientboundPostEffectsPacket(List.of(
                Identifier.parse("minecraft:invert"), Identifier.parse("pack:glow")
            )));
            life.add("after_set_two", idList(mc.player.getActivePostEffects()));

            listener.handlePostEffects(new ClientboundPostEffectsPacket(List.of()));
            life.add("after_set_empty", idList(mc.player.getActivePostEffects()));

            JsonObject respawns = new JsonObject();
            for (byte keep : new byte[]{0, 1, 2, 3}) {
                listener.handlePostEffects(new ClientboundPostEffectsPacket(List.of(
                    Identifier.parse("minecraft:invert"), Identifier.parse("pack:glow")
                )));
                CommonPlayerSpawnInfo target = (keep & 2) != 0 ? netherSpawn : overworldSpawn;
                listener.handleRespawn(new ClientboundRespawnPacket(target, keep));
                respawns.add("keep_" + keep, idList(mc.player.getActivePostEffects()));
            }
            life.add("after_respawn", respawns);

            // Second login after clearClientLevel (mc.player = null, as handleConfigurationStart does)
            listener.handlePostEffects(new ClientboundPostEffectsPacket(List.of(Identifier.parse("minecraft:invert"))));
            mc.player = null;
            listener.handleLogin(loginPacket);
            life.add("after_reconfigure_then_login", idList(mc.player.getActivePostEffects()));

            // Second login WITHOUT clearClientLevel (mc.player still non-null)
            listener.handlePostEffects(new ClientboundPostEffectsPacket(List.of(Identifier.parse("minecraft:invert"))));
            listener.handleLogin(loginPacket);
            life.add("after_repeat_login_without_clear", idList(mc.player.getActivePostEffects()));

            root.add("play_lifecycle", life);
        }

        return root;
    }

    static JsonObject recordEntityData(ProtocolInfo<ClientGamePacketListener> play) throws Exception {
        JsonObject root = new JsonObject();
        root.addProperty("game_version", "26.3");

        // 1. Registered serializer IDs in 26.3
        JsonArray registered = new JsonArray();
        for (int id = 0; id <= 64; id++) {
            if (EntityDataSerializers.getSerializer(id) != null) registered.add(id);
        }
        root.add("registered_serializer_ids", registered);
        root.addProperty("neg1_registered", EntityDataSerializers.getSerializer(-1) != null);
        root.addProperty("id_44_registered", EntityDataSerializers.getSerializer(44) != null);
        root.addProperty("id_200_registered", EntityDataSerializers.getSerializer(200) != null);

        // 2. Unknown serializers: decode via ClientboundSetEntityDataPacket.STREAM_CODEC and PacketDecoder+Connection
        JsonArray unknowns = new JsonArray();
        record UnknownCase(String name, int serializerId, boolean prependValidByte) {}
        for (var c : List.of(
            new UnknownCase("id_200_after_valid_byte", 200, true),
            new UnknownCase("id_44_just_past_max", 44, false),
            new UnknownCase("id_neg1", -1, false)
        )) {
            RegistryFriendlyByteBuf dataOnly = new RegistryFriendlyByteBuf(Unpooled.buffer(), ParityData.access);
            if (c.prependValidByte) {
                dataOnly.writeByte(0);
                dataOnly.writeVarInt(0); // BYTE
                dataOnly.writeByte(7);
            }
            dataOnly.writeByte(9);
            dataOnly.writeVarInt(c.serializerId);
            dataOnly.writeBytes(new byte[]{1, 2, 3});
            dataOnly.writeByte(0xFF);
            String entityDataHex = toHex(dataOnly);

            RegistryFriendlyByteBuf packetBody = new RegistryFriendlyByteBuf(Unpooled.buffer(), ParityData.access);
            packetBody.writeVarInt(1); // entity id
            packetBody.writeBytes(dataOnly.duplicate());
            String bodyHex = toHex(packetBody);

            String exClass = "none";
            String exMsg = "";
            boolean ok = false;
            try {
                ClientboundSetEntityDataPacket.STREAM_CODEC.decode(new RegistryFriendlyByteBuf(packetBody.duplicate(), ParityData.access));
                ok = true;
            } catch (Exception e) {
                exClass = e.getClass().getSimpleName();
                exMsg = e.getMessage();
            }

            // Also run through real PacketDecoder + Connection.exceptionCaught
            ByteBuf framed = Unpooled.buffer();
            // Get the wire id of SET_ENTITY_DATA by encoding an empty packet
            ByteBuf sample = encodeFrame(play, new ClientboundSetEntityDataPacket(1, List.of()));
            int setEntityDataId = new FriendlyByteBuf(sample).readVarInt();
            new FriendlyByteBuf(framed).writeVarInt(setEntityDataId);
            framed.writeBytes(packetBody.duplicate());

            RecordingConnection conn = new RecordingConnection();
            EmbeddedChannel ch = new EmbeddedChannel(new PacketDecoder<>(play), conn);
            ch.writeInbound(framed);
            String connDisconnectKey = conn.disconnectKey;
            String connDisconnectText = conn.disconnectText;
            ch.finishAndReleaseAll();

            JsonObject row = new JsonObject();
            row.addProperty("name", c.name);
            row.addProperty("serializer_id", c.serializerId);
            row.addProperty("entity_data_hex", entityDataHex);
            row.addProperty("packet_body_hex", bodyHex);
            row.addProperty("decoded_ok", ok);
            row.addProperty("exception_class", exClass);
            row.addProperty("exception_message", exMsg);
            row.addProperty("connection_disconnect_key", connDisconnectKey);
            row.addProperty("connection_disconnect_text", connDisconnectText);
            unknowns.add(row);
        }
        root.add("unknown_serializers", unknowns);

        // 3. Valid serializer 33 (OPTIONAL_GLOBAL_POS) and 34 (PAINTING_VARIANT registry + direct inline)
        JsonArray validCases = new JsonArray();
        Holder<PaintingVariant> regPainting = ParityData.access.lookupOrThrow(Registries.PAINTING_VARIANT).listElements().findFirst().orElseThrow();
        Holder<PaintingVariant> inlinePainting = Holder.direct(new PaintingVariant(
            2, 1, Identifier.parse("minecraft:kebab"), Optional.of(Component.literal("Kebab")), Optional.empty()
        ));

        record ValidCase(String name, List<SynchedEntityData.DataValue<?>> values) {}
        for (var c : List.of(
            new ValidCase("opt_global_pos_empty", List.of(
                new SynchedEntityData.DataValue<>(8, EntityDataSerializers.OPTIONAL_GLOBAL_POS, Optional.empty()),
                new SynchedEntityData.DataValue<>(0, EntityDataSerializers.BYTE, (byte) 5)
            )),
            new ValidCase("opt_global_pos_present", List.of(
                new SynchedEntityData.DataValue<>(8, EntityDataSerializers.OPTIONAL_GLOBAL_POS, Optional.of(GlobalPos.of(Level.OVERWORLD, new BlockPos(10, 64, -20)))),
                new SynchedEntityData.DataValue<>(0, EntityDataSerializers.BYTE, (byte) 5)
            )),
            new ValidCase("painting_variant_registry", List.of(
                new SynchedEntityData.DataValue<>(9, EntityDataSerializers.PAINTING_VARIANT, regPainting),
                new SynchedEntityData.DataValue<>(0, EntityDataSerializers.BYTE, (byte) 5)
            )),
            new ValidCase("painting_variant_inline", List.of(
                new SynchedEntityData.DataValue<>(9, EntityDataSerializers.PAINTING_VARIANT, inlinePainting),
                new SynchedEntityData.DataValue<>(0, EntityDataSerializers.BYTE, (byte) 5)
            ))
        )) {
            RegistryFriendlyByteBuf body = new RegistryFriendlyByteBuf(Unpooled.buffer(), ParityData.access);
            ClientboundSetEntityDataPacket pkt = new ClientboundSetEntityDataPacket(1, c.values);
            ClientboundSetEntityDataPacket.STREAM_CODEC.encode(body, pkt);
            String bodyHex = toHex(body);
            // Strip leading VarInt entity id (1 -> single byte 0x01) for EntityData::read tests
            RegistryFriendlyByteBuf stripped = new RegistryFriendlyByteBuf(body.duplicate(), ParityData.access);
            int entityId = stripped.readVarInt();
            String entityDataHex = toHex(stripped);

            // Round-trip decode with vanilla
            ClientboundSetEntityDataPacket back = ClientboundSetEntityDataPacket.STREAM_CODEC.decode(
                new RegistryFriendlyByteBuf(body.duplicate(), ParityData.access)
            );
            JsonObject row = new JsonObject();
            row.addProperty("name", c.name);
            row.addProperty("entity_id", entityId);
            row.addProperty("entity_data_hex", entityDataHex);
            row.addProperty("packet_body_hex", bodyHex);
            row.addProperty("entry_count", back.packedItems().size());
            row.addProperty("trailing_index", back.packedItems().get(1).id());
            row.addProperty("trailing_byte", ((Byte) back.packedItems().get(1).value()).intValue());
            validCases.add(row);
        }
        root.add("valid_serializers", validCases);

        return root;
    }

    public static void main(String[] args) throws Exception {
        ParityData.init();
        Class<?> u = Class.forName("sun.misc.Unsafe");
        Field uf = u.getDeclaredField("theUnsafe");
        uf.setAccessible(true);
        unsafe = uf.get(null);
        allocate = u.getMethod("allocateInstance", Class.class);

        Path outDir = Path.of(args[0]);
        Files.createDirectories(outDir);
        File gameDir = Files.createTempDirectory("sunrise-net-parity-oracle").toFile();

        ProtocolInfo<ClientGamePacketListener> play =
            GameProtocols.CLIENTBOUND_TEMPLATE.bind(RegistryFriendlyByteBuf.decorator(ParityData.access));

        var gson = new GsonBuilder().setPrettyPrinting().disableHtmlEscaping().create();
        Files.writeString(outDir.resolve("bundle.json"), gson.toJson(recordBundles(play, gameDir)) + "\n");
        Files.writeString(outDir.resolve("post_effects.json"), gson.toJson(recordPostEffects(play, gameDir)) + "\n");
        Files.writeString(outDir.resolve("entity_data.json"), gson.toJson(recordEntityData(play)) + "\n");
        System.out.println("NETWORK_PARITY_WITNESS_OK " + outDir);
    }
}
