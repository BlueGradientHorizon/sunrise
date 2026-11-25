/// A mod's camera (`native_camera_pose`): each non-finite component keeps vanilla's `view`.
pub(in crate::game) fn posed_camera(
    view: (glam::DVec3, f32, f32),
    position: [f64; 3],
    yaw: f32,
    pitch: f32,
) -> (glam::DVec3, f32, f32) {
    let position = std::array::from_fn(|i| {
        if position[i].is_finite() {
            position[i]
        } else {
            view.0[i]
        }
    });
    (
        glam::DVec3::from_array(position),
        if yaw.is_finite() { yaw } else { view.1 },
        if pitch.is_finite() { pitch } else { view.2 },
    )
}

impl Game<'_> {
    /// Opens `screen` (`Gui.setScreen`; `Screen::Playing` is vanilla's null).
    pub(in crate::game) fn set_screen(&mut self, screen: Screen) {
        self.set_screen_with(screen, |_| {});
    }

    /// Runs `screen_change` from `current` to `new`, then resolves the hooks' decision.
    pub(in crate::game) fn decide_screen(
        &self,
        current: &ScreenView<'_>,
        new: &ScreenView<'_>,
        closes: bool,
    ) -> ScreenDecision {
        self.screen_decision(self.screen_change(current, new, true, closes))
    }

    /// Fires `screen_opening` and `screen_closing` in order for a screen transition.
    pub(in crate::game) fn screen_change(
        &self,
        current: &ScreenView<'_>,
        new: &ScreenView<'_>,
        opens: bool,
        closes: bool,
    ) -> Option<Answered<ScreenOpening>> {
        // Asked before either screen draws: nothing beside it yet.
        let current = &crate::game::screen_backgrounds::without_regions(current);
        let new = &crate::game::screen_backgrounds::without_regions(new);
        let mut outcome = None;
        if opens {
            self.native_screen_opening(current, new, &mut outcome);
        }
        if closes
            && outcome
                .as_ref()
                .is_none_or(|a| a.answer != ScreenOpening::Cancel)
        {
            self.native_screen_closing(current);
        }
        outcome
    }

    /// Shows `title` (`Minecraft.setScreen(new TitleScreen())`) with the game-rule registry attached.
    pub(in crate::game) fn show_title(&mut self, mut title: sunrise_client::title::Title) {
        title.set_rule_registry(Arc::new(crate::game::api_values::game_rule_registry()));
        self.show_title_with_registry(title);
    }

    /// Admission for ordinary toasts; mandatory callback-panic diagnostics bypass mod admission.
    pub(in crate::game) fn admit_toast(
        &self,
        kind: mod_api::ToastKind,
        title: &mut Text,
        description: &mut Option<Text>,
    ) -> bool {
        // Only the receiving client owns toast presentation and its hook.
        if self.is_logical_server() {
            return false;
        }
        let mut cancel = false;
        self.native_toast_add(kind, title, description, &mut cancel);
        !cancel
    }

    /// Client tick target duration in ms (`Minecraft.getTickTargetMillis`), after mod hooks.
    pub(in crate::game) fn tick_target_millis(&self) -> f32 {
        let vanilla = self.clock.tick_target_millis();
        let mut millis = vanilla;
        self.native_tick_target_millis(&mut millis);
        if millis.is_finite() && millis > 0.0 {
            millis
        } else {
            vanilla
        }
    }

    /// Local player's heart kind (`Hud.HeartType.forPlayer`, then `player_heart_type`).
    pub(in crate::game) fn player_heart_kind(
        &mut self,
    ) -> Option<mod_api::client::hud_layout::HeartKind> {
        use mod_api::client::hud_layout::HeartKind;
        let p = self.player.as_ref()?;
        let effects = &self.potions.effects;
        let mut kind = if effects.get("poison").is_some() {
            HeartKind::Poisoned
        } else if effects.get("wither").is_some() {
            HeartKind::Withered
        } else if p.is_fully_frozen() {
            HeartKind::Frozen
        } else {
            HeartKind::Normal
        };
        self.native_player_heart_type(&mut kind);
        Some(kind)
    }

    /// Whether any mod listens to a hook that receives `ScreenView`.
    pub(in crate::game) fn screen_view_listened(&self) -> bool {
        use mod_api::HookId;
        [
            HookId::ScreenRenderPre,
            HookId::ScreenRenderBackground,
            HookId::ScreenRenderForeground,
            HookId::ScreenRenderPost,
            HookId::ScreenMouseButtonPressedPre,
            HookId::ScreenMouseButtonPressedPost,
            HookId::ScreenMouseButtonReleasedPre,
            HookId::ScreenMouseButtonReleasedPost,
            HookId::ScreenMouseDraggedPre,
            HookId::ScreenMouseDraggedPost,
            HookId::ScreenMouseScrolledPre,
            HookId::ScreenMouseScrolledPost,
            HookId::ScreenKeyPressedPre,
            HookId::ScreenKeyPressedPost,
            HookId::ScreenKeyReleasedPre,
            HookId::ScreenKeyReleasedPost,
            HookId::ScreenCharTypedPre,
            HookId::ScreenCharTypedPost,
            HookId::ScreenOpening,
            HookId::ScreenClosing,
            HookId::ScreenInitPost,
            HookId::ScreenRoom,
            HookId::ScreenExclusionAreas,
        ]
        .into_iter()
        .any(mod_api::__private::listened)
    }

    /// Whether a mod listens to one of the four screen render hooks.
    pub(in crate::game) fn screen_render_listened(&self) -> bool {
        use mod_api::HookId;
        [
            HookId::ScreenRenderPre,
            HookId::ScreenRenderBackground,
            HookId::ScreenRenderForeground,
            HookId::ScreenRenderPost,
        ]
        .into_iter()
        .any(mod_api::__private::listened)
    }

    /// `Render.Pre`: whether the screen draws.
    pub(in crate::game) fn screen_render_begin(
        &self,
        draw: &mut HudBatch,
        screen: &ScreenView<'_>,
    ) -> bool {
        let mut canceled = false;
        self.native_screen_render_pre(draw, screen, &mut canceled);
        !canceled
    }

    /// `Render.Post`, including when Pre cancelled the screen's drawing.
    pub(in crate::game) fn screen_render_end(&self, draw: &mut HudBatch, screen: &ScreenView<'_>) {
        self.native_screen_render_post(draw, screen);
    }

    /// Borrowed `ScreenView` of the open screen for this frame.
    pub(in crate::game) fn screen_view(&self, batch: &HudBatch, screen: Screen) -> ScreenView<'_> {
        self.screen_view_at(screen, self.screen_rect(batch, screen))
    }

    /// `screen_view` with an explicit panel `rect` for container layout.
    pub(in crate::game) fn screen_view_at(&self, screen: Screen, rect: [f32; 4]) -> ScreenView<'_> {
        // Only the inventory screen shows a server's menu.
        let menu_shown = screen == Screen::Inventory;
        let shown_native = menu_shown.then(|| self.shown_native_screen()).flatten();
        ScreenView::from(mod_api::__private::built::ScreenView {
            id: screen.id(),
            mod_screen: (screen == Screen::Mod).then(|| self.mod_screen()).flatten(),
            menu: self
                .remote_menu_type()
                .filter(|m| menu_shown && !m.is_empty())
                .or_else(|| {
                    shown_native
                        .map(|s| s.menu.name.as_str())
                        .filter(|m| !m.is_empty())
                }),
            title: self
                .remote
                .as_ref()
                .and_then(|r| r.menu_title_snapshot())
                .filter(|_| menu_shown)
                .or_else(|| shown_native.and_then(|s| s.menu.title.as_ref())),
            rect,
            slots: self.slot_count(),
            mouse: self.cursor(),
            partial_tick: self.clock.fraction() as f32,
            regions: self.screen_regions(screen),
        })
    }

    /// Button Post sees the original receiver and its answer; return the final handled bit.
    pub(in crate::game) fn screen_button_post(
        &self,
        receiver: &ObservedScreen,
        button: i32,
        pressed: bool,
        modifiers: u32,
        double_click: bool,
        handled: bool,
    ) -> bool {
        let screen = receiver.view();
        let mut result = TriState::Default;
        if pressed {
            self.native_screen_mouse_button_pressed_post(
                &screen,
                button,
                modifiers,
                double_click,
                handled,
                &mut result,
            );
        } else {
            self.native_screen_mouse_button_released_post(
                &screen,
                button,
                modifiers,
                handled,
                &mut result,
            );
        }
        result.or(handled)
    }

    /// Unhandled scroll/drag notices go to the receiver captured before handling.
    pub(in crate::game) fn screen_mouse_post(&self, post: ScreenMousePost) {
        if post.handled {
            return;
        }
        let screen = post.receiver.view();
        match post.kind {
            ScreenMousePostKind::Scroll(scroll) => {
                self.native_screen_mouse_scrolled_post(&screen, scroll, post.modifiers);
            }
            ScreenMousePostKind::Drag { button, delta } => {
                self.native_screen_mouse_dragged_post(&screen, button, delta, post.modifiers);
            }
        }
    }

    /// Next-frame position of container slot `slot` on open mod menu `menu`.
    pub(in crate::game) fn screen_slot(&self, menu: &str, slot: usize) -> Option<[f32; 2]> {
        let s = self.shown_native_screen().filter(|s| s.menu.name == menu)?;
        s.slot_moves.latest(&s.layout().slots, slot)
    }

    /// Moves, activates, or hides container slot `slot` on open mod menu `menu` from next frame.
    pub(in crate::game) fn set_screen_slot(
        &self,
        menu: &str,
        slot: usize,
        pos: Option<[f32; 2]>,
    ) -> Result<(), mod_api::ScreenSlotRefusal> {
        let s = self
            .shown_native_screen()
            .filter(|s| s.menu.name == menu)
            .ok_or(mod_api::ScreenSlotRefusal::NoScreen)?;
        s.slot_moves.set(&s.layout().slots, slot, pos)
    }

    /// Options > Mods rows for loaded code mods, after the content-only rows.
    pub(in crate::game) fn list_native_mods(&mut self) {
        let registered = crate::game::mods::collect_key_mappings(
            self.options.execution_role,
            mod_api::__private::key_mappings(),
        );
        let native = self.mods.native.clone();
        let mut rows: Vec<_> = self
            .settings
            .mods
            .installed
            .iter()
            .filter(|row| {
                mod_api::__private::mod_index(&row.id).is_some()
                    && !native.built().iter().any(|m| m.id == row.id)
            })
            .cloned()
            .collect();
        rows.extend(native.packages().iter().map(|package| {
            let m = &package.manifest;
            self.settings
                .mods
                .info
                .insert(m.id.clone(), sunrise_client::mod_list::ModInfo::of(m, None));
            Installed {
                id: m.id.clone(),
                title: format!("{} {}", m.name, m.version),
                description: m.description.clone(),
                problem: None,
                dev: false,
                native: None,
            }
        }));
        self.settings
            .mods
            .installed
            .retain(|row| !rows.iter().any(|code| code.id == row.id));
        for mut row in rows {
            let settings = self.native_page_settings(&row.id);
            let mappings: Vec<KeyMapping> = registered
                .iter()
                .filter(|(owner, _)| *owner == row.id)
                .map(|(_, k)| KeyMapping {
                    id: k.id.clone(),
                    label: k.label.clone(),
                    key: k.key.clone(),
                    modifiers: k.modifiers.bits,
                    context: crate::game::api_values::TryIntoEngine::try_into_engine(
                        &k.context,
                        &(),
                    )
                    .unwrap(),
                    category: k.category.clone(),
                })
                .collect();
            row.native = Some(NativePage { mappings, settings });
            self.settings.mods.installed.push(row);
        }
    }

    /// The settings mod `mod_id` registered, with their kept values, as its page shows them.
    pub(in crate::game) fn native_page_settings(&self, mod_id: &str) -> Vec<NativeSetting> {
        mod_api::__private::settings()
            .iter()
            .filter(|(owner, _)| *owner == mod_id)
            .map(|(_, spec)| {
                let kept = self.stored_setting(mod_id, &spec.key);
                let value = match (&spec.kind, spec.read(kept.as_deref())) {
                    (_, mod_api::SettingValue::Toggle(on)) => NativeSettingValue::Toggle(on),
                    (
                        mod_api::SettingKind::Choice { options, .. },
                        mod_api::SettingValue::Choice(index),
                    ) => NativeSettingValue::Choice {
                        options: options.clone(),
                        index,
                    },
                    (
                        mod_api::SettingKind::Range { min, max, step, .. },
                        mod_api::SettingValue::Range(value),
                    ) => NativeSettingValue::Range {
                        min: *min,
                        max: *max,
                        step: *step,
                        value,
                    },
                    (_, mod_api::SettingValue::Color(rgb)) => NativeSettingValue::Color(rgb),
                    _ => NativeSettingValue::Toggle(false),
                };
                NativeSetting {
                    key: spec.key.clone(),
                    label: spec.label.clone(),
                    tooltip: spec.tooltip.clone(),
                    value,
                }
            })
            .collect()
    }

    /// Persists a changed setting on native mod `i`'s page and fires `setting_changed`.
    pub(in crate::game) fn native_setting_event(&mut self, i: usize, j: usize) {
        let Some(row) = self.settings.mods.installed.get(i) else {
            return;
        };
        let Some(setting) = row.native.as_ref().and_then(|page| page.settings.get(j)) else {
            return;
        };
        let (id, key) = (row.id.clone(), setting.key.clone());
        let value = match &setting.value {
            NativeSettingValue::Toggle(on) => mod_api::SettingValue::Toggle(*on),
            NativeSettingValue::Choice { index, .. } => mod_api::SettingValue::Choice(*index),
            NativeSettingValue::Range { value, .. } => mod_api::SettingValue::Range(*value),
            NativeSettingValue::Color(rgb) => mod_api::SettingValue::Color(*rgb),
        };
        let kept = mod_api::__private::setting(&id, &key).and_then(|spec| spec.store(value));
        let Some(kept) = kept else {
            return;
        };
        if self.store_setting(&id, &key, &kept) {
            self.native_setting_changed(&id, &key);
        }
    }

    /// Native mod `i`'s page opened: its settings' kept values read again.
    pub(in crate::game) fn native_settings_refresh(&mut self, i: usize) {
        let Some(id) = self.settings.mods.installed.get(i).map(|m| m.id.clone()) else {
            return;
        };
        let settings = self.native_page_settings(&id);
        let row = self.settings.mods.installed.get_mut(i);
        if let Some(page) = row.and_then(|m| m.native.as_mut()) {
            page.settings = settings;
        }
    }

    /// Shader-pack params from native mods this frame, with non-finite values zeroed.
    pub(in crate::game) fn native_shader_params_now(&self, partial: f32) -> [[f32; 4]; 8] {
        let mut params = [[0.0; 4]; 8];
        self.native_shader_params(partial, &mut params);
        finite_params(params)
    }
}
