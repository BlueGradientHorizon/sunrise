//! Mod API GUI commands to and from the engine's: every variant, field for field.
#[cfg(test)]
use sunrise_gameplay_common::game::api_values::TryIntoApi;

macro_rules! gui_draw_value {
    ($draw:expr, $from:ident, $to:ident, $text:expr) => {
        match $draw {
            $from::Sprite {
                sprite,
                x,
                y,
                w,
                h,
                region,
                color,
                blend,
            } => $to::Sprite {
                sprite: sprite.clone(),
                x: *x,
                y: *y,
                w: *w,
                h: *h,
                region: *region,
                color: *color,
                blend: *blend,
            },
            $from::AtlasSprite {
                sprite,
                x,
                y,
                w,
                h,
                color,
            } => $to::AtlasSprite {
                sprite: sprite.clone(),
                x: *x,
                y: *y,
                w: *w,
                h: *h,
                color: *color,
            },
            $from::Blit {
                texture,
                blend,
                x,
                y,
                w,
                h,
                uv,
                src,
                tex,
                color,
            } => $to::Blit {
                texture: texture.clone(),
                blend: *blend,
                x: *x,
                y: *y,
                w: *w,
                h: *h,
                uv: *uv,
                src: *src,
                tex: *tex,
                color: *color,
            },
            $from::Fill {
                x0,
                y0,
                x1,
                y1,
                color,
            } => $to::Fill {
                x0: *x0,
                y0: *y0,
                x1: *x1,
                y1: *y1,
                color: *color,
            },
            $from::FillGradient {
                x0,
                y0,
                x1,
                y1,
                top,
                bottom,
            } => $to::FillGradient {
                x0: *x0,
                y0: *y0,
                x1: *x1,
                y1: *y1,
                top: *top,
                bottom: *bottom,
            },
            $from::Text {
                text,
                x,
                y,
                color,
                shadow,
            } => $to::Text {
                text: $text(text)?,
                x: *x,
                y: *y,
                color: *color,
                shadow: *shadow,
            },
            $from::Item {
                slot,
                x,
                y,
                seed,
                scale,
                pivot,
            } => $to::Item {
                slot: *slot,
                x: *x,
                y: *y,
                seed: *seed,
                scale: *scale,
                pivot: *pivot,
            },
            $from::PlayerFace {
                uuid,
                x,
                y,
                size,
                hat,
                flip,
                color,
            } => $to::PlayerFace {
                uuid: *uuid,
                x: *x,
                y: *y,
                size: *size,
                hat: *hat,
                flip: *flip,
                color: *color,
            },
            $from::Push(ops) => $to::Push(ops.clone()),
            $from::Pop => $to::Pop,
        }
    };
}

#[cfg(test)]
pub(in crate::game) fn gui_draw_to_api(
    draw: &sunrise_client_render::hud_layout::GuiDraw,
) -> Result<mod_api::client::hud_layout::GuiDraw, mod_api::text::TextFormError> {
    use mod_api::client::hud_layout::GuiDraw as Api;
    use sunrise_client_render::hud_layout::GuiDraw as Engine;
    Ok(gui_draw_value!(
        draw,
        Engine,
        Api,
        |t: &sunrise_game_data::text::Text| t.try_into_api()
    ))
}

pub(in crate::game) fn gui_draw_to_engine(
    draw: &mod_api::client::hud_layout::GuiDraw,
) -> Result<sunrise_client_render::hud_layout::GuiDraw, mod_api::text::TextFormError> {
    use mod_api::client::hud_layout::GuiDraw as Api;
    use sunrise_client_render::hud_layout::GuiDraw as Engine;
    Ok(gui_draw_value!(
        draw,
        Api,
        Engine,
        sunrise_sim::api_text::to_engine
    ))
}

#[cfg(test)]
mod tests {
    use super::{gui_draw_to_api, gui_draw_to_engine};
    use serde_json::json;
    use sunrise_client_render::hud_layout::{Blend, GuiDraw as D, PoseOp};
    use sunrise_game_data::text::Text;

    #[test]
    fn gui_commands_keep_every_variant_field_order_and_text_source() {
        let text = Text::from(
            json!({"translate":"test.hud", "fallback":"%s", "with":[{"text":"name"}], "bold":false, "test:meta":{"n":1}}),
        );
        let commands = vec![
            D::Sprite {
                sprite: String::from("test:sprite").into(),
                x: -1,
                y: 2,
                w: 3,
                h: 4,
                region: Some([5, 6, 7, 8]),
                color: 0x12345678,
                blend: Blend::Crosshair,
            },
            D::AtlasSprite {
                sprite: "test:atlas".into(),
                x: 11,
                y: 12,
                w: 13,
                h: 14,
                color: 15,
            },
            D::Blit {
                texture: "test:texture".into(),
                blend: Blend::Nausea,
                x: 21,
                y: 22,
                w: 23,
                h: 24,
                uv: [0.25, 0.5],
                src: [26, 27],
                tex: [28, 29],
                color: 30,
            },
            D::Fill {
                x0: -31,
                y0: 32,
                x1: 33,
                y1: 34,
                color: 35,
            },
            D::FillGradient {
                x0: 36,
                y0: -37,
                x1: 38,
                y1: 39,
                top: 0x8000_0040,
                bottom: 0xFF00_0041,
            },
            D::Text {
                text: text.clone(),
                x: 41,
                y: 42,
                color: 43,
                shadow: false,
            },
            D::Item {
                slot: 40,
                x: 51,
                y: 52,
                seed: -53,
                scale: [0.5, 1.5],
                pivot: [55., 56.],
            },
            D::PlayerFace {
                uuid: 0x0102_0304_0506_0708_090a_0b0c_0d0e_0f10,
                x: -57,
                y: 58,
                size: 59,
                hat: false,
                flip: true,
                color: 0x8000_0060,
            },
            D::Push(vec![PoseOp::Translate(61., 62.), PoseOp::Scale(0.5, 1.5)]),
            D::Pop,
        ];
        let api = commands
            .iter()
            .map(gui_draw_to_api)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let actual: Result<Vec<D>, mod_api::text::TextFormError> =
            api.iter().map(gui_draw_to_engine).collect();
        let actual = actual.unwrap();
        assert_eq!(actual, commands);
        let D::Text { text: actual, .. } = &actual[5] else {
            panic!("text command")
        };
        assert_eq!(actual.component(), text.component());
    }
}
