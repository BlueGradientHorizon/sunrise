package dev.sunriselab.parity;

import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.mojang.serialization.JsonOps;
import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import net.minecraft.ReportedException;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.RegistryDataLoader;
import net.minecraft.resources.RegistryOps;
import net.minecraft.server.RegistryLayer;
import net.minecraft.server.dialog.Dialog;
import net.minecraft.server.packs.PackLocationInfo;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.PathPackResources;
import net.minecraft.server.packs.repository.PackSource;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.tags.TagLoader;

/**
 * Runs the real dialog codec on a corpus file ({"cases": [...], "packs": [...]}).
 * A case ({"name", "dialog"}) is decoded by Dialog.DIRECT_CODEC over the vanilla
 * data pack's registries; its line is {"name", "accept", "error"}, the error
 * being the codec's own message. A pack ({"name", "files": {path: json}}) is
 * written over the vanilla pack and loaded by RegistryDataLoader as a world is
 * opened; its line is {"name", "accept", "error"}, the error being the
 * "registry/id: message" lines of the load report.
 */
public final class VanillaDialogCodecOracle {
    public static void main(String[] args) throws Exception {
        ParityData.init();
        var ops = RegistryOps.create(JsonOps.INSTANCE, ParityData.lookup);
        JsonObject corpus = JsonParser.parseString(Files.readString(Path.of(args[0]))).getAsJsonObject();
        Path scratch = Files.createTempDirectory("dialog-packs");
        try (PrintWriter out = new PrintWriter(Files.newBufferedWriter(Path.of(args[1]), StandardCharsets.UTF_8))) {
            for (JsonElement entry : corpus.getAsJsonArray("cases")) {
                JsonObject one = entry.getAsJsonObject();
                var result = Dialog.DIRECT_CODEC.parse(ops, one.get("dialog"));
                JsonObject line = new JsonObject();
                line.addProperty("name", one.get("name").getAsString());
                line.addProperty("accept", result.result().isPresent());
                if (result.result().isEmpty()) line.addProperty("error", result.error().orElseThrow().message());
                out.println(new Gson().toJson(line));
            }
            var repo = ServerPacksSource.createVanillaTrustedRepository();
            repo.reload();
            repo.setSelected(List.of("vanilla"));
            int index = 0;
            for (JsonElement entry : corpus.getAsJsonArray("packs")) {
                JsonObject one = entry.getAsJsonObject();
                Path root = scratch.resolve("pack" + index++);
                Files.createDirectories(root);
                Files.writeString(root.resolve("pack.mcmeta"), "{\"pack\":{\"description\":\"t\",\"min_format\":[102,0],\"max_format\":[102,0]}}");
                for (Map.Entry<String, JsonElement> file : one.getAsJsonObject("files").entrySet()) {
                    Path path = root.resolve(file.getKey());
                    Files.createDirectories(path.getParent());
                    Files.writeString(path, new Gson().toJson(file.getValue()));
                }
                var packs = new ArrayList<>(repo.openAllSelected());
                packs.add(new PathPackResources(new PackLocationInfo("test", Component.literal("t"), PackSource.WORLD, Optional.empty()), root));
                var resources = new MultiPackResourceManager(PackType.SERVER_DATA, packs);
                var layers = RegistryLayer.createRegistryAccess();
                var staticTags = TagLoader.loadTagsForExistingRegistries(resources, layers.getLayer(RegistryLayer.STATIC));
                var context = TagLoader.buildUpdatedLookups(layers.getAccessForLoading(RegistryLayer.WORLD), staticTags);
                JsonObject line = new JsonObject();
                line.addProperty("name", one.get("name").getAsString());
                try {
                    RegistryDataLoader.load(resources, context, RegistryDataLoader.WORLD_REGISTRIES, Runnable::run).join();
                    line.addProperty("accept", true);
                } catch (RuntimeException error) {
                    Throwable cause = error.getCause() != null ? error.getCause() : error;
                    if (!(cause instanceof ReportedException reported)) throw error;
                    String report = reported.getReport().getFriendlyReport(net.minecraft.ReportType.CRASH);
                    int at = report.indexOf("Errors:");
                    int end = report.indexOf("Stacktrace", at);
                    line.addProperty("accept", false);
                    line.addProperty("error", report.substring(at + "Errors:".length(), end).trim().replaceAll("\\s*\\n\\s*", "\n"));
                }
                out.println(new Gson().toJson(line));
            }
        }
    }
}
