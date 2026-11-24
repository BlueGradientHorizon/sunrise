#[allow(unused_imports)]
pub(in crate::game::server_side) use super::raw_backlog::Received;
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::remote_chunks::{
    ChunkDecoder, ChunkFormat, Next, RemoteLight,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::remote_vehicles::PassengerSeat;
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::render_tick::{
    mob_tracking_body, player_tracking_body,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::villager_client::builtin_registries;
#[allow(unused_imports)]
pub(in crate::game::server_side) use super::{
    Dim, Game, Hit, Mining, Screen, item_use::ticks_left, level_sounds::LevelSound,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::api_values::TryIntoApi;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::collision_source::Mover;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::BACKLOG_LIMIT;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::CHUNK_PACKETS_PER_FRAME;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::ClientMenu;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::DryChanges;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::DryPlace;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::DrySound;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::DryUse;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::ItemUseAim;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::Lerp;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::MOUNT_MENU;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::NoTerrain;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::OwnProfile;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::PendingCauldron;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::PendingItem;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::PlayerTextures;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::Remote;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::RemoteDimensionEntry;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::RemoteInputWorld;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::RemoteUseWorld;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::SHIFT_INPUT;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::ServicesKeys;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::TICK_DECODE_WAIT;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::Tracked;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::boat_interact_passes;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::break_prediction::Prediction;
#[allow(unused_imports)]
#[cfg(test)]
pub(in crate::game::server_side) use crate::game::remote::chat::Decoration;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::chat_after_event;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::chat_received::{
    QueuedChat, QueuedChatKind,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::client_fluid_step;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::client_has_level;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::client_millis;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::client_view_distance;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::dry_level::DryLevel;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::entity_sound_state;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::face_direction;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::general::RemoteGeneral;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::hand_id;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::hand_operated;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::hold_server_fire;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::input_ordinal;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::inventory::{
    CREATIVE_SYNCED, Predicted, changed_slots, creative_sync_slot, creative_sync_stacks,
    mark_synced, menu_items, predict_clicks, predict_clicks_with,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::item_frame_interact_passes;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::item_has_use;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::keep_client_state;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::look_at_rotation;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::menu_title;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::minecarts::CartLerp;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::mob_pos_rot;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::movement::MovePacket;
#[allow(unused_imports)]
#[cfg(test)]
pub(in crate::game::server_side) use crate::game::remote::movement::stepped_move;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::named_hand_operated;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::number_format;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::object_skin_changed;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::place_interpolated;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::player_voice_pitch;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::position_packet_pitch;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::remote_object_swings;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::remote_player_head_turn;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::remote_text;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::remote_use_swings;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::seat_passengers;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::selector_metadata::hurt_sound_for;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::shape_meets;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::skin_entries;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::snap_target;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::snapshot_value;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::synced_local_use;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::use_ends_click;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::wrap_degrees;
#[allow(unused_imports)]
pub(in crate::game::server_side) use crate::game::remote::*;
#[allow(unused_imports)]
pub(in crate::game::server_side) use mod_api::InteractionKey;
#[allow(unused_imports)]
pub(in crate::game::server_side) use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::first_person::{Hand, SwingAnimation};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_client::{
    Result,
    blocks::Fluid,
    chunk::Chunk,
    containers::{BlockEntity, Furnace, FurnaceKind},
    entities::{Experience, ExperienceOrb, ItemEntity},
    hud::HudBatch,
    items::ItemStack,
    join_seq::JoinSeq,
    lan::data::{BIOME_REGISTRY, client_biomes},
    menu::Input,
    mobs::{Mob, MobType, Who},
    motion::{MotionWorld, PlayerMotion},
    net::{Event, Session},
    particles::TrackingTarget,
    random_source,
    save::SaveDir,
    screen_effects,
    sprint::Keys,
    streaming::{ChunkGenerator, Proto, Streaming},
    tab_list::SPECTATOR,
    text::Text,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_core::{
    geometry::Vec3, profile::AccountProperty, random::RandomSource,
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_protocol::{
    entity_data::{ITEM_ENTITY_ITEM, Value},
    item::{Hashed, Stack},
    packets::{
        Empty,
        play::{self, Clientbound as C, Serverbound as S},
    },
    registry::{Builtin, DimensionType, Dynamic, SyncedNames, ceil_log2},
    rsa::RsaPublicKey,
    secure_chat::{self, Signature},
    text::{Language, plain},
};
#[allow(unused_imports)]
pub(in crate::game::server_side) use sunrise_server_files::profiles::ServicesKeySet;
#[allow(unused_imports)]
pub(in crate::game::server_side) use {
    sunrise_core::nbt::Tag,
    sunrise_mod_runtime::attachments::{Holder, ResidentAttachments},
    sunrise_world::{
        level::InteractionResult,
        registry::BlockStateId,
        stepped_interpolation::{Path, PosRot, Start, SteppedInterpolation},
    },
};

#[path = "general_server.rs"]
pub(in crate::game) mod general;

#[path = "recipe_properties_server.rs"]
pub(in crate::game) mod recipe_properties;

#[path = "session_server.rs"]
pub(in crate::game) mod session;

#[path = "tables_server.rs"]
pub(in crate::game) mod tables;

#[cfg(test)]
#[path = "interaction_event_tests_server.rs"]
pub(in crate::game) mod interaction_event_tests;

#[cfg(test)]
#[path = "passage_cross_tests_server.rs"]
pub(in crate::game) mod passage_cross_tests;

#[cfg(test)]
#[path = "riptide_tests_server.rs"]
pub(in crate::game) mod riptide_tests;

#[cfg(test)]
pub(in crate::game) mod tests {
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use super::super::api_values::TryIntoApi;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use super::Game;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use super::{
        boat_interact_passes, chat_after_event, item_frame_interact_passes, remote_object_swings,
    };
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::remote::tests::connect_menu_test;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::remote::tests::connect_play_test;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::remote::tests::finish_play_test;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::remote::tests::offer_test_payloads;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::remote::tests::spawn_menu_test;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use crate::game::remote::tests::*;
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use sunrise_client::{
        containers::{Beacon, BlockEntity},
        items::ItemStack,
    };
    #[allow(unused_imports)]
    pub(in crate::game::server_side) use sunrise_core::nbt::Tag;

    pub(in crate::game) mod click_ending_tests {
        include!("click_ending_tests_server.rs");
    }

    pub(in crate::game) mod dig_abort_tests {
        include!("dig_abort_tests_server.rs");
    }
}

#[cfg(test)]
#[path = "movement_input_tests_server.rs"]
pub(in crate::game) mod movement_input_tests;

#[cfg(test)]
#[path = "placement_prediction_tests_server.rs"]
pub(in crate::game) mod placement_prediction_tests;
