pub use crate::parts::{DeclarationError, LoadingMessage};
mod __private {
    pub use crate::parts::{
        DeclarationKind, DeclarationRefusal, DeclarationSink, DeclarationStage,
        RegistrationDiagnostics, registration_context_with_diagnostics, registration_warnings,
        take_declaration_refusals,
    };
}
