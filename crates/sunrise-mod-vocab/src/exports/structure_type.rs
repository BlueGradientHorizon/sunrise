pub use world::structure_type::TerrainAdjustment;
pub use world::structure_type::{
    BiomeId, ChunkPos, DimensionPadding, FoundStructure, GenerationContext, HeightmapType,
    JigsawLayout, JigsawReach, JigsawSettings, LiquidSettings, NoiseBlock, NoiseColumn,
    PieceBeardifier, PieceContext, PoolAliasBinding, StructureError, StructurePiece, StructureType,
};
mod __private {
    /// The registered structure types, for the executable to hand to the engine's
    /// `StructureRegistry` before any world opens.
    pub use crate::world::structure_type::{biome_id, jigsaw_layout, structure_type_declarations};
}
