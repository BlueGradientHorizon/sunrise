package dev.sunriselab.parity;

import com.mojang.blaze3d.vertex.PoseStack;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.renderer.LevelRenderer;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.entity.EntityRenderDispatcher;
import net.minecraft.client.renderer.entity.state.EntityRenderState;
import net.minecraft.client.renderer.state.level.CameraRenderState;
import net.minecraft.client.renderer.state.level.LevelRenderState;
import net.minecraft.world.level.entity.EntityAccess;
import net.minecraft.world.level.entity.EntityLookup;
import net.minecraft.world.phys.Vec3;

/** Runtime lookup and submission; the scene supplies admissions and already-visible bodies. */
public final class VanillaEntityDrawOrderOracle {
    private static final class Body extends EntityRenderState {
        final int id;

        Body(int id) {
            this.id = id;
        }
    }

    private static final class Recorder extends EntityRenderDispatcher {
        final List<Integer> submitted = new ArrayList<>();

        Recorder() {
            super(null, null, null, null, null, null, null, null, null, null, null);
        }

        @Override
        public <S extends EntityRenderState> void submit(
            S state, CameraRenderState camera, double x, double y, double z,
            PoseStack pose, SubmitNodeCollector output
        ) {
            submitted.add(((Body) state).id);
        }
    }

    public static void main(String[] args) throws Exception {
        var recorder = new Recorder();
        // Invoke the real submit loop without the renderer constructor's GPU resources.
        var unsafeClass = Class.forName("sun.misc.Unsafe");
        var unsafeField = unsafeClass.getDeclaredField("theUnsafe");
        unsafeField.setAccessible(true);
        var renderer = (LevelRenderer) unsafeClass.getMethod("allocateInstance", Class.class)
            .invoke(unsafeField.get(null), LevelRenderer.class);
        var dispatcher = LevelRenderer.class.getDeclaredField("entityRenderDispatcher");
        dispatcher.setAccessible(true);
        dispatcher.set(renderer, recorder);
        var submit = LevelRenderer.class.getDeclaredMethod(
            "submitEntities", PoseStack.class, LevelRenderState.class, SubmitNodeCollector.class);
        submit.setAccessible(true);

        var lookup = new EntityLookup<EntityAccess>();
        EntityAccess player = null;
        for (String line : Files.readAllLines(Path.of(args[0]))) {
            String[] words = line.split(" ");
            switch (words[0]) {
                case "login" -> {
                    player = VanillaEntityLookupOrderOracle.entity(Integer.parseInt(words[1]));
                    lookup.add(player);
                }
                case "respawn" -> {
                    // Same ClientLevel: addEntity replaces the old player under the same ID.
                    lookup.remove(player);
                    player = VanillaEntityLookupOrderOracle.entity(player.getId());
                    lookup.add(player);
                }
                case "add" -> lookup.add(VanillaEntityLookupOrderOracle.entity(Integer.parseInt(words[1])));
                case "position" -> {} // Position correction does not reinsert the player.
                case "frame" -> {
                    var state = new LevelRenderState();
                    state.cameraRenderState.pos = Vec3.ZERO;
                    // The visible extraction step preserves lookup order; all fixture bodies are visible.
                    lookup.getAllEntities().forEach(e -> state.entityRenderStates.add(new Body(e.getId())));
                    recorder.submitted.clear();
                    submit.invoke(renderer, new PoseStack(), state, null);
                    System.out.println("entity-draw-order: " + recorder.submitted);
                }
                default -> throw new IllegalArgumentException(line);
            }
        }
    }
}
