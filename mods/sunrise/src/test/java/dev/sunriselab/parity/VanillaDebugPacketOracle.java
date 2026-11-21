package dev.sunriselab.parity;

import io.netty.buffer.Unpooled;
import java.nio.file.*;
import java.util.*;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.network.protocol.game.ClientboundDebugSamplePacket;
import net.minecraft.network.protocol.game.ServerboundDebugSubscriptionRequestPacket;
import net.minecraft.server.Bootstrap;
import net.minecraft.util.debug.DebugSubscriptions;
import net.minecraft.util.debugchart.RemoteDebugSampleType;

/** Debug sample / subscription request bodies through the native stream codecs. */
public final class VanillaDebugPacketOracle {
    static String hex(RegistryFriendlyByteBuf buf) {
        byte[] bytes = new byte[buf.readableBytes()]; buf.readBytes(bytes); buf.release();
        return HexFormat.of().formatHex(bytes);
    }

    public static void main(String[] args) throws Exception {
        net.minecraft.SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var access = RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY);
        var out = new ArrayList<String>();
        for (long[] sample : new long[][]{{}, {50_000_000L, -1L}, {0L, Long.MAX_VALUE, Long.MIN_VALUE, 1L << 40}}) {
            var buf = new RegistryFriendlyByteBuf(Unpooled.buffer(), access);
            ClientboundDebugSamplePacket.STREAM_CODEC.encode(buf, new ClientboundDebugSamplePacket(sample, RemoteDebugSampleType.TICK_TIME));
            out.add("{\"packet\":\"sample\",\"sample\":" + Arrays.toString(sample) + ",\"kind\":" + RemoteDebugSampleType.TICK_TIME.ordinal() + ",\"wire\":\"" + hex(buf) + "\"}");
        }
        for (var set : List.of(Set.<net.minecraft.util.debug.DebugSubscription<?>>of(), Set.<net.minecraft.util.debug.DebugSubscription<?>>of(DebugSubscriptions.DEDICATED_SERVER_TICK_TIME))) {
            var buf = new RegistryFriendlyByteBuf(Unpooled.buffer(), access);
            ServerboundDebugSubscriptionRequestPacket.STREAM_CODEC.encode(buf, new ServerboundDebugSubscriptionRequestPacket(set));
            var ids = new ArrayList<Integer>();
            for (var s : set) ids.add(BuiltInRegistries.DEBUG_SUBSCRIPTION.getId(s));
            out.add("{\"packet\":\"request\",\"ids\":" + ids + ",\"wire\":\"" + hex(buf) + "\"}");
        }
        Files.write(Path.of(args[0]), out);
    }
}
