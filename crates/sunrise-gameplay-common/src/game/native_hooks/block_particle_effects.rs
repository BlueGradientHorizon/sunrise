//! Decide whether a block Part handles the client effect before sampling the default shape.
use super::*;
use crate::game::api_values::block_state::block_listens;
use sunrise_core::{geometry::Direction, position::BlockPos};
