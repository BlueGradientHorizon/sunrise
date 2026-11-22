package dev.sunriselab.parity;

import java.nio.file.Path;
import net.minecraft.SharedConstants;
import net.minecraft.core.UUIDUtil;
import net.minecraft.nbt.NbtIo;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.NbtUtils;
import net.minecraft.nbt.TagParser;

/** A regular player save with an active effect but no saved modifier for it. */
public final class WaypointEffectSave {
    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        var player = TagParser.parseCompoundFully("""
            {Pos:[2.5d,-60.0d,0.5d],Dimension:"minecraft:overworld",Health:20.0f,playerGameType:1,
             active_effects:[{id:"minecraft:invisibility",amplifier:0,duration:200,
                              hidden_effect:{amplifier:0,duration:-1}}]}
            """);
        var uuid = UUIDUtil.createOfflinePlayerUUID("Bob");
        player.put("UUID", UUIDUtil.CODEC.encodeStart(NbtOps.INSTANCE, uuid).getOrThrow());
        NbtIo.writeCompressed(NbtUtils.addCurrentDataVersion(player), Path.of(args[0]).resolve(uuid + ".dat"));
    }
}
