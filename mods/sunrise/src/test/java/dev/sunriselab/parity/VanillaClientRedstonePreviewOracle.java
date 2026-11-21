package dev.sunriselab.parity;

import java.util.List;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.multiplayer.prediction.BlockStatePredictionHandler;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.util.RandomSource;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.ObserverBlock;
import net.minecraft.world.level.block.RepeaterBlock;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.LevelChunk;

/** Runs the shipped client placement and neighbor methods without a game window. */
public final class VanillaClientRedstonePreviewOracle {
    // The setBlock case needs storage, but not a loaded renderer or a network connection.
    // Stub the chunk write; ClientLevel.setBlock and Level.setBlock still execute unchanged.
    static final class Chunk extends LevelChunk {
        BlockState stored;
        Chunk() { super(null, null); }
        @Override public BlockState setBlockState(BlockPos pos, BlockState state, int flags) {
            var old = stored;
            stored = state;
            return old;
        }
    }

    static final class World extends ClientLevel {
        Chunk chunk;
        int outputNotifications;
        World() { super(null, null, null, null, 0, 0, null, false, 0, 0); }
        @Override public boolean isClientSide() { return true; }
        @Override public boolean hasNeighborSignal(BlockPos pos) { return true; }
        @Override public BlockState getBlockState(BlockPos pos) {
            return pos.equals(BlockPos.ZERO) ? chunk.stored : Blocks.AIR.defaultBlockState();
        }
        @Override public boolean isInValidBounds(BlockPos pos) { return pos.equals(BlockPos.ZERO); }
        @Override public LevelChunk getChunkAt(BlockPos pos) { return chunk; }
        @Override public void setBlocksDirty(BlockPos pos, BlockState old, BlockState state) {}
        @Override public void updateNeighbourForOutputSignal(BlockPos pos, Block block) {
            outputNotifications++;
        }
        @Override public boolean removeBlock(BlockPos pos, boolean moved) {
            throw new AssertionError("client placement must not remove powered TNT");
        }
        @Override public void blockEvent(BlockPos pos, Block block, int event, int direction) {
            throw new AssertionError("client placement must not start a piston move");
        }
    }

    public static void main(String[] args) throws Exception {
        net.minecraft.SharedConstants.tryDetectVersion();
        net.minecraft.server.Bootstrap.bootStrap();
        var field = Class.forName("sun.misc.Unsafe").getDeclaredField("theUnsafe");
        field.setAccessible(true);
        var unsafe = field.get(null);
        var allocate = unsafe.getClass().getMethod("allocateInstance", Class.class);
        var world = (World) allocate.invoke(unsafe, World.class);
        world.chunk = (Chunk) allocate.invoke(unsafe, Chunk.class);
        var predictionHandler = ClientLevel.class.getDeclaredField("blockStatePredictionHandler");
        predictionHandler.setAccessible(true);
        predictionHandler.set(world, new BlockStatePredictionHandler());

        var pos = BlockPos.ZERO;
        var air = Blocks.AIR.defaultBlockState();
        world.chunk.stored = air;
        for (var block : List.of(Blocks.TNT, Blocks.PISTON, Blocks.STICKY_PISTON)) {
            block.defaultBlockState().onPlace(world, pos, air, false);
        }

        var observer = Blocks.OBSERVER.defaultBlockState().setValue(ObserverBlock.POWERED, true);
        world.chunk.stored = observer;
        observer.onPlace(world, pos, air, true);
        if (world.getBlockState(pos) != observer) {
            throw new AssertionError("client placement must preserve a powered observer");
        }

        var repeater = Blocks.REPEATER.defaultBlockState()
            .setValue(RepeaterBlock.FACING, Direction.NORTH).setValue(RepeaterBlock.LOCKED, true);
        world.chunk.stored = repeater;
        var updated = repeater.updateShape(world, world, pos, Direction.WEST,
            pos.west(), air, RandomSource.create(0));
        if (updated != repeater) {
            throw new AssertionError("a client side-neighbor shape update must preserve repeater lock");
        }

        world.chunk.stored = air;
        var cake = Blocks.CAKE.defaultBlockState();
        if (!cake.hasAnalogOutputSignal()
                || !world.setBlock(pos, cake, Block.UPDATE_NEIGHBORS | Block.UPDATE_KNOWN_SHAPE)
                || world.getBlockState(pos) != cake || world.outputNotifications != 0) {
            throw new AssertionError("client setBlock must write the source without notifying output neighbors");
        }
        System.out.println("Vanilla client placement and redstone update checks passed.");
    }
}
