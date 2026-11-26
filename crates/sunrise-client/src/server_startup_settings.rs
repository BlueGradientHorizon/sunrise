use std::{collections::HashMap, path::Path};
use sunrise_hooks::settings::engine;
use sunrise_level::sim_settings::{MAX_RENDER_DISTANCE, SimSettings};
use sunrise_level::tick_mode::ParallelTickMode;
use sunrise_mod_package::mod_list::ModList;

pub(super) struct Workers {
    configured: u32,
    battery: bool,
}

impl Workers {
    pub(super) fn count(&self) -> usize {
        if self.configured > 0 {
            return self.configured as usize;
        }
        let auto = sunrise_threads::jobs::default_threads();
        if self.battery { auto.div_ceil(2) } else { auto }
    }
}

pub(super) fn load(path: &Path) -> (SimSettings, Workers) {
    const EXTRAS: &str = "sunrise-extras.txt";
    const GRAPHICS: &str = "sunrise-graphics.txt";
    let parent = path.parent();
    let extras = parent.map_or_else(String::new, |dir| match engine::read(&dir.join(EXTRAS)) {
        Ok(None) => String::new(),
        Ok(Some(read)) => {
            if read.lossy {
                log::warn!("Sunrise: {EXTRAS} has bytes that aren't UTF-8, so it won't be saved over");
            }
            read.text
        }
        Err(error) => {
            log::warn!("Sunrise: {EXTRAS} could not be read, so it won't be saved over: {error}");
            String::new()
        }
    });
    let graphics = parent.and_then(|dir| engine::read(&dir.join(GRAPHICS)).ok().flatten());
    engine::load(&mut |file| match file {
        EXTRAS => Some(extras.clone()),
        GRAPHICS => graphics.as_ref().map(|read| read.text.clone()),
        _ => parent.and_then(|dir| engine::read(&dir.join(file)).ok().flatten().map(|read| read.text)),
    });

    let mut settings = SimSettings::default();
    let mut workers = Workers { configured: 0, battery: false };
    for (key, value) in graphics.iter().flat_map(|read| read.text.lines()).filter_map(setting_pair) {
        match key {
            "parallelTick" => settings.parallel_tick = ParallelTickMode::parse(value),
            "workers" => workers.configured = value.parse::<u32>().unwrap_or(workers.configured).min(64),
            "batteryMode" => workers.battery = read_flag(value, workers.battery),
            _ => {}
        }
    }
    for (key, value) in extras.lines().filter_map(setting_pair) {
        match key {
            sunrise_core::no_ai::KEY => settings.no_ai = read_flag(value, settings.no_ai),
            "agentControl" => settings.agent_control = read_flag(value, settings.agent_control),
            _ => {}
        }
    }
    if let Some(dir) = parent {
        settings.mods = ModList::load(&sunrise_level::logging::shared_dir(dir));
    }
    if let Ok(bytes) = std::fs::read(path) {
        apply_options(&mut settings, &String::from_utf8_lossy(&bytes));
    }
    (settings, workers)
}

fn setting_pair(line: &str) -> Option<(&str, &str)> {
    line.split_once(':').map(|(key, value)| (key.trim(), value.trim()))
}

fn read_flag(value: &str, default: bool) -> bool {
    value.parse().unwrap_or(default)
}

fn apply_options(settings: &mut SimSettings, text: &str) {
    let raw: HashMap<_, _> = text.lines().filter_map(|line| line.split_once(':')).collect();
    let int = |key| raw.get(key).and_then(|value| value.trim().parse::<i32>().ok());
    let flag = |key| raw.get(key).and_then(|value| value.trim().parse::<bool>().ok()).unwrap_or(false);
    if let Some(distance) = int("renderDistance").filter(|distance| (2..=MAX_RENDER_DISTANCE).contains(distance)) {
        settings.render_distance = distance;
    }
    if let Some(distance) = int("simulationDistance").filter(|distance| (5..=32).contains(distance)) {
        settings.simulation_distance = distance;
    }
    if let Some(distance) = raw.get("entityDistanceScaling")
        .and_then(|value| value.trim().parse::<f64>().ok())
        .filter(|distance| (0.5..=5.0).contains(distance))
    {
        settings.entity_distance = (distance * 4.0).trunc() / 4.0;
    }
    settings.auto_jump = flag("autoJump");
    settings.main_hand_left = raw.get("mainHand").is_some_and(|value| {
        matches!(value.trim(), r#""left""# | "left")
    });
    settings.chat_visibility = int("chatVisibility").map_or(0, |value| value.rem_euclid(3) as u8);
    settings.operator_items_tab = flag("operatorItemsTab");
    settings.reduced_debug_info = flag("reducedDebugInfo");
    if let Some(language) = raw.get("lang") {
        settings.language = (*language).to_owned();
    }
}
