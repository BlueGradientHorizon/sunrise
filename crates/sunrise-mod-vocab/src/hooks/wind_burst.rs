pub trait Hooks {
    /// Before a wind burst interacts with blocks, `wind_burst(dimension, center, blocks)` receives the shuffled candidate positions. Remove positions to spare those blocks; entity knockback and the shuffle's random draws are unchanged. Not called when mob_griefing prevents a breeze from interacting with blocks. Bundles need "hooks": @API:wind_burst@.
    #[hook(target = "sunrise_gameplay::game::native_hooks::wind_burst::ServerGame<'_>::native_wind_burst", 
        dispatch = "flat",
        side = "server",
        contract = "collect",
        out = "blocks",
        neoforge = "none (vanilla wind-charge block interaction; NeoForge adds no event)"
    )]
    fn wind_burst(dimension: &str, center: [f64; 3], blocks: &mut Vec<[i32; 3]>);
}
