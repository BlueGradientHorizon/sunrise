pub fn install_client_hooks() {
    crate::game::api_values::install_stack_hooks();
    if !sunrise_sim::native_client_hooks::install(
        sunrise_sim::native_client_hooks::NativeClientHooks {
            menu_still_valid: crate::game::api_values::menu_still_valid,
            menu_still_valid_listens: |menu| {
                mod_api::__private::menu_listens(mod_api::HookId::MenuStillValid, menu)
            },
            living_heal: |context, entity, amount, cancel| {
                if !mod_api::__private::listened(mod_api::HookId::LivingHeal) {
                    return;
                }
                let reader = sunrise_level::api_context::LoopReader(context);
                let context = reader.context();
                mod_api::dispatch_living_heal(&context, entity, amount, cancel);
            },
            living_breathe_heard: || mod_api::__private::listened(mod_api::HookId::LivingBreathe),
            has_mods: || !mod_api::__private::mods().is_empty(),
            player_name_format: |context, username, displayname| {
                use crate::game::TryIntoApi;
                if !mod_api::__private::listened(mod_api::HookId::PlayerNameFormat) {
                    return;
                }
                let reader = sunrise_level::api_context::LoopReader(context);
                let context = reader.context();
                let Ok(username) = username.try_into_api() else {
                    return;
                };
                let Ok(mut converted) = displayname.try_into_api() else {
                    return;
                };
                mod_api::dispatch_player_name_format(&context, &username, &mut converted);
                if let Ok(converted) = sunrise_sim::api_text::to_serialized_engine(&converted) {
                    *displayname = converted;
                }
            },
            player_tab_list_name_format: |context, player, name, display_name| {
                use crate::game::TryIntoApi;
                if !mod_api::__private::listened(mod_api::HookId::PlayerTabListNameFormat) {
                    return;
                }
                let reader = sunrise_level::api_context::LoopReader(context);
                let context = reader.context();
                let Ok(mut converted) = display_name.try_into_api() else {
                    return;
                };
                mod_api::dispatch_player_tab_list_name_format(
                    &context,
                    player,
                    name,
                    &mut converted,
                );
                if let Ok(converted) = converted
                    .as_ref()
                    .map(sunrise_sim::api_text::to_serialized_engine)
                    .transpose()
                {
                    *display_name = converted;
                }
            },
            living_drown: |context,
                           entity,
                           kind,
                           is_drowning,
                           damage_amount,
                           bubble_count,
                           cancel| {
                if !mod_api::__private::listened(mod_api::HookId::LivingDrown) {
                    return;
                }
                let reader = sunrise_level::api_context::LoopReader(context);
                let context = reader.context();
                mod_api::dispatch_living_drown(
                    &context,
                    entity,
                    kind,
                    is_drowning,
                    damage_amount,
                    bubble_count,
                    cancel,
                );
            },
            living_fall: |context, entity, kind, distance, multiplier, cancel| {
                if !mod_api::__private::listened(mod_api::HookId::LivingFall) {
                    return;
                }
                let reader = sunrise_level::api_context::LoopReader(context);
                let context = reader.context();
                mod_api::dispatch_living_fall(&context, entity, kind, distance, multiplier, cancel);
            },
            living_jump: |context, entity, kind, velocity| {
                if !mod_api::__private::listened(mod_api::HookId::LivingJump) {
                    return;
                }
                let reader = sunrise_level::api_context::LoopReader(context);
                let context = reader.context();
                mod_api::dispatch_living_jump(&context, entity, kind, velocity);
            },
            living_jump_power: |context, entity, kind, power| {
                if !mod_api::__private::listened(mod_api::HookId::LivingJumpPower) {
                    return;
                }
                let reader = sunrise_level::api_context::LoopReader(context);
                let context = reader.context();
                mod_api::dispatch_living_jump_power(&context, entity, kind, power);
            },
            living_speed: |context, entity, kind, speed| {
                if !mod_api::__private::listened(mod_api::HookId::LivingSpeed) {
                    return;
                }
                let reader = sunrise_level::api_context::LoopReader(context);
                let context = reader.context();
                mod_api::dispatch_living_speed(&context, entity, kind, speed);
            },
            living_flying_speed: |context, entity, kind, speed| {
                if !mod_api::__private::listened(mod_api::HookId::LivingFlyingSpeed) {
                    return;
                }
                let reader = sunrise_level::api_context::LoopReader(context);
                let context = reader.context();
                mod_api::dispatch_living_flying_speed(&context, entity, kind, speed);
            },
            block_jump_factor: |context, state, factor| {
                crate::game::block_jump_factor(context, state, factor);
            },
            block_speed_factor: |context, state, factor| {
                crate::game::block_speed_factor(context, state, factor);
            },
            is_ladder: |context, entity, pos, state, ladder| {
                crate::game::is_ladder(context, entity, pos, state, ladder);
            },
            makes_open_trapdoor_above_climbable:
                crate::game::native_trapdoor_ladder::makes_open_trapdoor_above_climbable,
            friction: crate::game::native_surface::friction,
            collision_shape: |context, pos, state, query, shape| {
                crate::game::native_collision_shape::collision_shape(
                    context, pos, state, query, shape,
                );
            },
            collision_shape_refused: crate::game::native_collision_shape::refused,
            player_stays_on_ground_surface: |context, entity, staying| {
                if !mod_api::__private::listened(mod_api::HookId::PlayerStaysOnGroundSurface) {
                    return;
                }
                let reader = sunrise_level::api_context::LoopReader(context);
                let context = reader.context();
                mod_api::dispatch_player_stays_on_ground_surface(&context, entity, staying);
            },
            ender_entity_teleport: |context, entity, kind, from, target, cancel| {
                if !mod_api::__private::listened(mod_api::HookId::EnderEntityTeleport) {
                    return;
                }
                let reader = sunrise_level::api_context::LoopReader(context);
                let context = reader.context();
                mod_api::dispatch_ender_entity_teleport(
                    &context, entity, kind, from, target, cancel,
                );
            },
            fluid_flow: |context, fluid, pos, flow| {
                if !mod_api::__private::listened(mod_api::HookId::FluidFlow) {
                    return;
                }
                let reader = sunrise_level::api_context::LoopReader(context);
                let context = reader.context();
                mod_api::dispatch_fluid_flow(&context, fluid, pos, flow);
            },
            sound_type: |context, pos, state, entity, cause, sound| {
                crate::game::native_block_sounds::sound_type(
                    context, pos, state, entity, cause, sound,
                );
            },
            can_grindstone_repair: crate::game::native_item_menu_parts::can_grindstone_repair,
            is_combine_repairable: crate::game::native_item_menu_parts::is_combine_repairable,
            can_fit_inside_container_items:
                crate::game::native_item_menu_parts::can_fit_inside_container_items,
            item_name: crate::game::native_item_menu_parts::item_name,
        },
    ) {
        log::warn!("Native client hooks already installed; keeping the first callbacks");
    }
}
