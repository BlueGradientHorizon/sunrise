package dev.sunriselab.parity;

import com.google.gson.Gson;
import com.mojang.authlib.GameProfile;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import net.minecraft.SharedConstants;
import net.minecraft.gametest.framework.GameTestServer;
import net.minecraft.network.Connection;
import net.minecraft.network.protocol.Packet;
import net.minecraft.network.protocol.PacketFlow;
import net.minecraft.network.protocol.game.ServerboundInteractPacket;
import net.minecraft.network.protocol.game.ServerboundPlayerLoadedPacket;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.level.ClientInformation;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.server.network.CommonListenerCookie;
import net.minecraft.server.network.ServerGamePacketListenerImpl;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntitySpawnReason;
import net.minecraft.world.entity.EntityType;
import net.minecraft.world.entity.EntityTypes;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.entity.Mob;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.GameType;
import net.minecraft.world.level.entity.PersistentEntitySectionManager;
import net.minecraft.world.level.entity.Visibility;
import net.minecraft.world.level.storage.LevelStorageSource;
import net.minecraft.world.phys.Vec3;

/** Sends Interact to the unmodified 26.3 server listener on its server thread. */
public final class VanillaSteerableBoardingOracle {
    private record Observation(List<String> seats, boolean riding, int held, boolean saddled) {}

    private static Observation scene(GameTestServer server, EntityType<? extends Mob> kind,
                                     boolean occupied, boolean saddled, Item held, boolean secondary) {
        ServerLevel level = server.overworld();
        Mob vehicle = kind.create(level, EntitySpawnReason.COMMAND);
        vehicle.snapTo(0.5, 100.0, 2.5, 0, 0);
        if (saddled) vehicle.setItemSlot(EquipmentSlot.SADDLE, new ItemStack(Items.SADDLE));
        if (!level.addFreshEntity(vehicle) || level.getEntity(vehicle.getId()) != vehicle) {
            throw new AssertionError("vehicle missing from the level lookup");
        }
        Mob passenger = EntityTypes.CHICKEN.create(level, EntitySpawnReason.COMMAND);
        if (occupied) {
            passenger.snapTo(0.5, 100.0, 2.5, 0, 0);
            if (!level.addFreshEntity(passenger) || !passenger.startRiding(vehicle, true, false)) {
                throw new AssertionError("mob did not board for fixture setup");
            }
        }
        ServerPlayer player = new ServerPlayer(server, level,
            new GameProfile(UUID.randomUUID(), "BoardingPlayer"), ClientInformation.createDefault());
        var listener = new ServerGamePacketListenerImpl(server, new Connection(PacketFlow.SERVERBOUND), player,
                CommonListenerCookie.createInitial(player.getGameProfile(), false)) {
            @Override
            public void send(Packet<?> packet) {} // No client socket; the observation reads live entities.
        };
        player.snapTo(0.5, 100.0, 0.5, 0, 0);
        player.setGameMode(GameType.SURVIVAL);
        if (held != null) player.setItemInHand(InteractionHand.MAIN_HAND, new ItemStack(held));
        new ServerboundPlayerLoadedPacket().handle(listener);
        new ServerboundInteractPacket(vehicle.getId(), InteractionHand.MAIN_HAND,
            new Vec3(0.0, 0.45, 0.0), secondary).handle(listener);
        var seats = vehicle.getPassengers().stream().map(rider -> {
            if (rider == passenger) return "mob";
            if (rider == player) return "player";
            throw new AssertionError("unexpected passenger");
        }).sorted().toList();
        var result = new Observation(seats, player.getVehicle() == vehicle, player.getMainHandItem().getCount(),
            vehicle.getItemBySlot(EquipmentSlot.SADDLE).is(Items.SADDLE));
        player.stopRiding();
        if (occupied) passenger.discard();
        vehicle.discard();
        return result;
    }

    private static void cases(Map<String, Observation> rows, GameTestServer server, String label,
                              EntityType<? extends Mob> kind, Item food) {
        rows.put(label + "/empty", scene(server, kind, false, true, null, false));
        rows.put(label + "/occupied", scene(server, kind, true, true, null, false));
        rows.put(label + "/secondary", scene(server, kind, false, true, null, true));
        rows.put(label + "/occupied_feed", scene(server, kind, true, true, food, false));
        rows.put(label + "/occupied_saddle", scene(server, kind, true, false, Items.SADDLE, false));
    }

    @SuppressWarnings("unchecked")
    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        try (var access = LevelStorageSource.createDefault(Path.of(args[0])).createAccess("boarding-oracle")) {
            var server = GameTestServer.create(Thread.currentThread(), access,
                ServerPacksSource.createPackRepository(access), Optional.empty(), false, 0);
            var init = GameTestServer.class.getDeclaredMethod("initServer");
            init.setAccessible(true);
            init.invoke(server);
            // There is no tick loop. Put the fixture chunk in the server's entity lookup.
            var field = ServerLevel.class.getDeclaredField("entityManager");
            field.setAccessible(true);
            var manager = (PersistentEntitySectionManager<Entity>) field.get(server.overworld());
            manager.updateChunkStatus(new ChunkPos(0, 0), Visibility.TRACKED);
            var rows = new LinkedHashMap<String, Observation>();
            cases(rows, server, "pig", EntityTypes.PIG, Items.CARROT);
            cases(rows, server, "strider", EntityTypes.STRIDER, Items.WARPED_FUNGUS);
            Files.writeString(Path.of(args[1]), new Gson().toJson(rows));
        }
        System.exit(0);
    }
}
