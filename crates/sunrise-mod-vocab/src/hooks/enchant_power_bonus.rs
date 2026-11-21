//! The `enchant_power_bonus` hook: declarations the mod API generator reads (see `hooks/mod.rs`).

pub trait Hooks {
    /// The enchanting power the supplied `state` at `pos` gives the table at `table_pos`
    /// (`IBlockExtension.getEnchantPowerBonus`). `power` starts at 1.0 for a block in
    /// `#minecraft:enchantment_power_provider`, 0.0 otherwise. A non-finite answer is ignored.
    ///
    /// `level` is the read-only level for a live offer or explicit query. An explicit query can
    /// occur during another callback and can ask about a state that doesn't occupy `pos`, with
    /// either position outside the table's usual bookshelf scan. A repeated active query refuses.
    /// The table, not the provider, applies transmitter, sum, truncation and offer-cost rules.
    ///
    /// `level` is `None` when rebuilding the client particle snapshot at world entry, parts
    /// reload or after received tags and their listeners have been published. Both positions are
    /// the origin for this state-only snapshot. Particles use the cached answers, not a per-frame
    /// dispatch. Any nonzero answer qualifies its state when the intervening block transmits.
    #[hook(
        side = "both",
        version = 3,
        contract = "collect",
        out = "power",
        scope = "block",
        key = "state",
        host = "read",
        neoforge = "IBlockExtension.getEnchantPowerBonus, IBlockStateExtension.getEnchantPowerBonus"
    )]
    fn enchant_power_bonus();
}
