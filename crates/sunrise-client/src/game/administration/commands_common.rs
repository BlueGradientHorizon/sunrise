//! Common command types and data-only helpers shared by server and client modules.
use super::Game;
use sunrise_client::{
    command::CommandError,
    items::ItemStack,
    text::{self, Style, Text},
};
use sunrise_core::{
    geometry::Vec3,
    nbt::Tag,
    version::{BUILD_TIME, DATA_VERSION, STABLE, VANILLA_VERSION, VERSION_ID},
};

pub(in crate::game) type R<T> = Result<T, CommandError>;

/// An entity a selector found.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(in crate::game) enum Target {
    /// The player in place: the host's, or the guest whose turn it is.
    Player,
    /// Another player of a hosted world or server: a guest by connection.
    Guest(u32),
    Mob(u32),
    Item(u32),
    /// An experience orb, by its entity id.
    Orb(u32),
    /// A projectile, by its entity id.
    Projectile(u32),
    Stand(u32),
    Frame(u32),
    Painting(u32),
    Boat(u32),
    Cart(u32),
    /// An entity of a type Sunrise does not simulate (markers, displays, interactions...).
    Other(u32),
    /// Active evoker fangs, including command-loaded ones.
    Fangs(u32),
    /// An End crystal, by its entity id (`minecraft:end_crystal`).
    Crystal(u32),
    /// The Ender Dragon, by its entity id (`minecraft:ender_dragon`).
    Dragon(u32),
    /// One of the dragon's eight `EnderDragonPart`s, by its own entity id.
    DragonPart(u32),
    /// A primed TNT, by its entity id (`minecraft:tnt`).
    Tnt(u32),
    /// A cushion, by its entity id (`minecraft:cushion`).
    Cushion(u32),
    /// A falling block, by its entity id (`minecraft:falling_block`).
    Falling(u32),
    /// A firework rocket, by its entity id (`minecraft:firework_rocket`).
    Rocket(u32),
    /// A thrown trident, by its entity id (`minecraft:trident`).
    Trident(u32),
    /// An ominous item spawner, by its entity id (`minecraft:ominous_item_spawner`).
    Spawner(u32),
    /// A simulated lingering, creeper or dragon area-effect cloud.
    Cloud(u32),
    /// A live lightning bolt (`minecraft:lightning_bolt`).
    Bolt(u32),
    /// A general entity row, by its entity id (#792).
    General(u32),
    /// A cast fishing hook (`minecraft:fishing_bobber`), which lives with its caster.
    Hook(u32),
    /// The fence knot a lead tie made, by entity id.
    Knot(u32),
}

/// `Entity.setXRot`: wrap before clamping the pitch.
pub(in crate::game) fn entity_pitch(pitch: f32) -> f32 {
    (pitch % 360.0).clamp(-90.0, 90.0)
}

/// `PositionMoveRotation.calculateAbsolute` and `TeleportCommand.performTeleport`.
pub(in crate::game) fn teleport_velocity(
    velocity: Vec3,
    relative: [bool; 3],
    fall_flying: bool,
) -> Vec3 {
    Vec3::new(
        if relative[0] { velocity.x } else { 0.0 },
        if relative[1] && fall_flying {
            velocity.y
        } else {
            0.0
        },
        if relative[2] { velocity.z } else { 0.0 },
    )
}

/// An entity of a type Sunrise does not simulate: raw compound lives in `persist.raw`.
#[derive(Clone, Debug)]
pub(in crate::game) struct OtherEntity {
    /// Entity-owned registration metadata; content clones are detached.
    pub(in crate::game) attachment_lifetime: sunrise_mod_runtime::attachments::AttachmentLifetime,
    pub(in crate::game) join: sunrise_client::join_seq::JoinSeq,
    pub id: u32,
    pub kind: String,
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub item_transformation: Option<super::persist::ItemDisplayTransformation>,
    /// A `minecraft:interaction` saved settings; None for every other kind.
    pub interaction: Option<super::interactions::InteractionState>,
    /// Only explicitly loaded leash knots own this BlockAttachedEntity check counter.
    pub knot_check: Option<u8>,
    /// A summoned or loaded `minecraft:eye_of_ender`'s flight; None for every other kind.
    pub eye: Option<super::end::SummonedEye>,
    /// What `entity_size` answered at its last size refresh (`EntityEvent.Size`).
    pub(in crate::game) size_answer: Option<sunrise_client::pose::SizeAnswer>,
}

sunrise_client::impl_joined!(OtherEntity);
sunrise_mod_runtime::impl_attachment_lifetime!(OtherEntity);

pub(in crate::game) fn err(key: &str, args: &[&dyn std::fmt::Display]) -> CommandError {
    CommandError::tr(key, args)
}

/// Format and validate a command failure as the sink sees it, including any timestamp.
pub(in crate::game) fn formatted_command_failure(message: Text, timestamp: Option<Text>) -> Text {
    use serde_json::Value;
    use sunrise_core::nbt;

    let format = |message| {
        let failure = Text::empty().append(message).with_color(text::RED);
        match &timestamp {
            Some(prefix) => prefix.clone().append(failure),
            None => failure,
        }
    };
    let output = format(message);
    let serializes = || {
        let component = output.component_view();
        let mut limits = nbt::Limits::default();
        // Check depth before recursing into the component converters.
        let mut pending = vec![(component.as_ref(), 0)];
        while let Some((node, depth)) = pending.pop() {
            match node {
                Value::Object(_) | Value::Array(_) if depth >= limits.max_depth => return false,
                Value::Object(fields) => pending.extend(fields.values().map(|v| (v, depth + 1))),
                Value::Array(values) => pending.extend(values.iter().map(|v| (v, depth + 1))),
                _ => {}
            }
        }
        let tag = if timestamp.is_some() {
            sunrise_client::scoreboard::component_tag(&component)
        } else {
            sunrise_client::lan::data::component_nbt(&component)
        };
        if timestamp.is_some() {
            // The saved component sits under chunk root, block_entities/Entities, and entry.
            limits.max_depth = nbt::Limits::saved().max_depth - 3;
        }
        // Use the strict writer so overlong strings and deep wrappers are refused.
        nbt::write_network(&tag, limits).is_ok()
    };
    if serializes() {
        output
    } else {
        format(Text::tr("command.failed", &[]))
    }
}

/// Exclude natural fence knots from targets a command moves or kills (#13036).
pub(in crate::game) fn without_knots(targets: Vec<Target>) -> R<Vec<Target>> {
    let found = !targets.is_empty();
    let kept: Vec<Target> = targets
        .into_iter()
        .filter(|t| !matches!(t, Target::Knot(_)))
        .collect();
    if found && kept.is_empty() {
        return Err(err("argument.entity.notfound.entity", &[]));
    }
    Ok(kept)
}

/// `EntityType.getDescriptionId`: `entity.<namespace>.<path>`.
pub(in crate::game) fn entity_name_key(id: &str) -> String {
    let (namespace, path) = id.split_once(':').unwrap_or(("minecraft", id));
    format!("entity.{namespace}.{}", path.replace('/', "."))
}

pub(in crate::game) fn texts(args: &[Text]) -> Vec<Text> {
    args.to_vec()
}

/// String.format(Locale.ROOT, "%.<decimals>f"): half-up decimal rounding.
pub(in crate::game) fn percent_f(v: f64, decimals: usize) -> String {
    sunrise_core::java_number::format_fixed(v, decimals)
}

/// String.format(Locale.ROOT, "%f").
pub(in crate::game) fn format_double(v: f64) -> String {
    percent_f(v, 6)
}

/// `EntitySelectorParser.createRotationPredicate`: wrapped angle range check.
pub(in crate::game) fn rotation_in((lo, hi): (Option<f64>, Option<f64>), angle: f32) -> bool {
    let min = wrap_degrees(lo.unwrap_or(0.0) as f32);
    let max = wrap_degrees(hi.unwrap_or(359.0) as f32);
    let angle = wrap_degrees(angle);
    if min > max {
        angle >= min || angle <= max
    } else {
        angle >= min && angle <= max
    }
}

/// `Mth.wrapDegrees`.
pub(in crate::game) fn wrap_degrees(angle: f32) -> f32 {
    let wrapped = angle % 360.0;
    if wrapped >= 180.0 {
        wrapped - 360.0
    } else if wrapped < -180.0 {
        wrapped + 360.0
    } else {
        wrapped
    }
}

/// Format the clickable context line under a syntax error, if the error has one.
pub(in crate::game) fn format_syntax_failure_context(
    e: &CommandError,
    command: &str,
) -> Option<Text> {
    let window = e.context_window()?;
    let suggest = Style {
        click: Some(text::ClickEvent::SuggestCommand(format!("/{command}"))),
        ..Style::default()
    };
    let mut context = Text::empty();
    if window.ellipsis {
        context.push("...");
    }
    context.push(window.before.as_str());
    if !window.after.is_empty() {
        context.push(
            Text::literal(window.after)
                .with_color(text::RED)
                .underlined(),
        );
    }
    context.push(
        Text::translate("command.context.here", &[])
            .with_color(text::RED)
            .italic(),
    );
    Some(context.with_color(text::GRAY).with_style(&suggest))
}

fn num(n: impl std::fmt::Display) -> Text {
    Text::literal(n.to_string())
}

impl Game<'_> {
    /// `ItemStack.getDisplayName`: hover name in brackets, coloured and carrying show_item hover.
    pub(in crate::game) fn stack_display_name(&self, s: &ItemStack) -> Text {
        let item = &*s.item;
        let empty = item == "minecraft:air" || s.count == 0;
        let custom = s.data().and_then(|data| data.custom_name.as_ref());
        let hover = self.stack_hover_name_text(s);
        // Component.empty().append(getHoverName()), italic for a custom name.
        let mut name = Text::empty().append(hover);
        if custom.is_some() {
            name = name.italic();
        }
        let wrapped = Text::translate("chat.square_brackets", &[name]);
        if empty {
            return wrapped;
        }

        // HoverEvent.ShowItem: the count only when it is not one.
        let mut shown = serde_json::json!({"action": "show_item", "id": item});
        if s.count != 1 {
            shown["count"] = s.count.into();
        }
        // ItemStackTemplate.fromNonEmptyStack keeps the stack's component patch.
        if let Some(patch) = sunrise_client::item_data::encode_components(s, self.items())
            && let Ok(components) = sunrise_core::json_nbt::tag_to_json(&patch)
        {
            shown["components"] = components;
        }
        wrapped
            .with_color(self.stack_rarity_color(s))
            .with_component_fields(vec![("hover_event".into(), shown)])
    }

    /// `BlockEntity.saveWithoutMetadata` of the block entity at `pos`, without id or position.
    pub(in crate::game) fn block_entity_tag(&self, pos: [i32; 3]) -> Option<Tag> {
        let e = self.world.block_entities.get(&(pos[0], pos[1], pos[2]))?;
        let name = self.data_block_name(pos);
        let mut tag =
            self.encode_live_block_entity((pos[0], pos[1], pos[2]), e, &name, self.res.items());
        if let Tag::Compound(c) = &mut tag {
            for key in ["id", "x", "y", "z", "keepPacked"] {
                c.remove(&key.into());
            }
        }
        Some(tag)
    }

    /// `EntityType.getDimensions().eyeHeight()` from `entity-types.json`, cached once.
    pub(in crate::game) fn type_eye_height(&self, ty: &str) -> f64 {
        static EYE_HEIGHTS: std::sync::OnceLock<std::collections::HashMap<String, f64>> =
            std::sync::OnceLock::new();
        EYE_HEIGHTS
            .get_or_init(|| {
                std::fs::read(self.options.resources.join("entity-types.json"))
                    .ok()
                    .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
                    .and_then(|v| {
                        v.as_object().map(|o| {
                            o.iter()
                                .filter_map(|(k, t)| {
                                    Some((k.clone(), f64::from(t["eye_height"].as_f64()? as f32)))
                                })
                                .collect()
                        })
                    })
                    .unwrap_or_default()
            })
            .get(ty)
            .copied()
            .unwrap_or(0.0)
    }

    /// `VersionCommand.dumpVersion`'s lines: Sunrise reports the version it implements.
    pub(in crate::game) fn version_lines(&self) -> Vec<Text> {
        let protocol = sunrise_protocol::PROTOCOL_VERSION;
        let mut lines: Vec<Text> = [
            ("commands.version.id", vec![Text::literal(VERSION_ID)]),
            (
                "commands.version.name",
                vec![Text::literal(VANILLA_VERSION)],
            ),
            ("commands.version.data", vec![num(DATA_VERSION)]),
            ("commands.version.series", vec![Text::literal("main")]),
            (
                "commands.version.protocol",
                vec![num(protocol), Text::literal(format!("0x{protocol:x}"))],
            ),
            (
                "commands.version.build_time",
                vec![Text::literal(BUILD_TIME)],
            ),
            (
                "commands.version.pack.resource",
                vec![Text::literal("97.1")],
            ),
            ("commands.version.pack.data", vec![Text::literal("121.0")]),
        ]
        .into_iter()
        .map(|(key, args)| Text::translate(key, &args))
        .collect();
        lines.push(Text::translate(
            if STABLE {
                "commands.version.stable.yes"
            } else {
                "commands.version.stable.no"
            },
            &[],
        ));
        lines
    }

    /// `Player.getDisplayName`: formatted team name with suggest, hover, and insertion.
    pub(in crate::game) fn player_text(&self, name: &str, uuid: u128) -> Text {
        let shown = match self.score.board.players_team(name) {
            Some(team) => team.formatted_name(Text::literal(name)),
            None => Text::literal(name),
        };
        let mut shown = shown.with_component_fields(vec![
            (
                "click_event".into(),
                serde_json::json!({"action": "suggest_command", "command": format!("/tell {name} ")}),
            ),
            (
                "hover_event".into(),
                serde_json::json!({"action": "show_entity", "id": "minecraft:player",
                    "uuid": sunrise_protocol::crypto::uuid_string(uuid), "name": name}),
            ),
            ("insertion".into(), name.into()),
        ]);
        sunrise_client::lan::hub::native_player_name_format(
            &self.loop_context,
            &Text::literal(name),
            &mut shown,
        );
        shown
    }

    /// `ItemStack.getMaxStackSize`: the `max_stack_size` component, else the item's.
    pub(in crate::game) fn stack_max_size(&self, s: &ItemStack) -> u32 {
        if let Some(Tag::Int(n)) = s.data().and_then(|d| d.other("minecraft:max_stack_size")) {
            return (*n).clamp(1, 99) as u32;
        }
        self.items().get(&s.item).map_or(64, |i| i.max_stack)
    }

    /// The player's movement collides with the border as it is now (`CollisionGetter.borderCollision`).
    pub(in crate::game) fn player_border(&mut self) {
        if let Some(p) = self.player.as_mut() {
            p.border = Some(self.border.geometry().clone());
        }
    }

    pub(in crate::game) fn data_block_name(&self, p: [i32; 3]) -> String {
        let state = self.world.block(p[0], p[1], p[2]);
        self.pack
            .assets
            .registry
            .state(state)
            .map_or_else(|| "minecraft:air".into(), |s| s.name.to_string())
    }
}
