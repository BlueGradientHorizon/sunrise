use sunrise_game_data::text::Text;
use sunrise_sim::merchant;

#[derive(Clone)]
pub(in crate::game) struct MerchantState {
    /// The merchant's side of the menu: offers, experience, level, the
    /// selection hint and active offer (`sunrise_client::merchant::Session`).
    pub(in crate::game) session: Box<merchant::Session>,
    /// The offer list's scroll offset.
    pub(in crate::game) scroll: usize,
    /// The scroller is held by the mouse (`isDragging`).
    pub(in crate::game) dragging: bool,
    /// The offer button that keeps focus after a click (drawn highlighted).
    pub(in crate::game) focused: Option<usize>,
    /// The trading mob (its offers and experience are written back on
    /// close) and its custom name.
    pub(in crate::game) mob: Option<u32>,
    /// `Merchant.getDisplayName()`: the trading mob's custom name with its
    /// style, else nothing (the profession's name is the default).
    pub(in crate::game) name: Option<Text>,
}

/// `Merchant.getDisplayName`: the merchant's custom name (style and all), else
/// the profession's or the wandering trader's.
pub(in crate::game) fn merchant_display_name(
    progress: bool,
    profession: &str,
    custom: Option<Text>,
) -> Text {
    custom.unwrap_or_else(|| {
        if progress {
            Text::translate_with_fallback(
                &format!("entity.minecraft.villager.{profession}"),
                profession,
                &[],
            )
        } else {
            Text::translate_with_fallback(
                "entity.minecraft.wandering_trader",
                "Wandering Trader",
                &[],
            )
        }
    })
}

impl MerchantState {
    /// `Merchant.showProgressBar`: villagers only.
    pub(in crate::game) fn progress(&self) -> bool {
        self.session.show_progress()
    }
}
