//! A command argument type whose arguments declare a suggestion provider: the engine asks that
//! provider for the argument alone, and for a compiled guest it reaches the guest as a nested
//! handle. One candidate does not match the typed text, so the engine's own filtering shows too;
//! the other carries a tooltip built from the whole request, so a lost prefix, argument or sender
//! shows up on either target.
use mod_api::{
    CommandArgumentError, CommandArgumentKind, CommandArgumentType, CommandSender,
    CommandSuggestion, SuggestionProvider, SuggestionReply, SuggestionRequest,
};
use std::sync::Arc;

/// The provider: two candidates, the first a match carrying what the request held.
pub struct Warps;

impl SuggestionProvider for Warps {
    fn suggest(&self, request: &SuggestionRequest) -> SuggestionReply {
        let earlier: Vec<String> = request
            .arguments
            .iter()
            .map(|argument| format!("{}={}", argument.name, argument.text))
            .collect();
        let sender = request.sender.as_ref().map_or_else(
            || "none".to_owned(),
            |sender| {
                format!(
                    "{}={}:{}",
                    sender.name(),
                    sender.uuid(),
                    sender.permission().level()
                )
            },
        );
        SuggestionReply::new([
            CommandSuggestion::with_tooltip(
                "press",
                format!("{}|{}|{}", request.prefix, earlier.join(","), sender),
            ),
            // The engine keeps only the candidates that match the typed text.
            CommandSuggestion::new("furnace"),
        ])
    }
}

/// The type: its own `suggest` must never answer, because it declares [`Warps`].
pub struct WarpsArgument;

impl CommandArgumentType for WarpsArgument {
    fn kind(&self) -> CommandArgumentKind {
        CommandArgumentKind::Word
    }

    fn parse(&self, input: &str) -> Result<String, CommandArgumentError> {
        Ok(input.to_owned())
    }

    fn suggest(&self, _: Option<&CommandSender>, _: &str) -> Vec<CommandSuggestion> {
        panic!("`suggest` is not called for an argument that declares a provider")
    }

    fn suggestion_provider(&self) -> Option<Arc<dyn SuggestionProvider>> {
        Some(Arc::new(Warps))
    }
}
