//! Attachment declarations and named refusals through the generated transport.
use super::*;

pub(super) struct AttachmentDeclarationHooks;
impl AttachmentDeclarationHooks {
    pub(super) fn register(
        &self,
        registrar: &mut dyn mod_api::Registrar,
        _: &mod_api::RegistrationContext,
    ) {
        registrar
            .register_attachment_type("wire_test:mana", false)
            .unwrap();
        // Registration has a Registrar frame, not a world Host. Metadata validation
        // must still cross and return the actual calling package's named error.
        assert_eq!(
            mod_api::AttachmentDeclarations::validate_attachment_type(
                &mod_api::wire::guest::Guest,
                "wire_test",
                "missing",
                false
            ),
            Err(mod_api::AttachmentError::Undeclared {
                package_id: "wire_test".into(),
                attachment_id: "wire_test:missing".into()
            })
        );
    }
}

#[test]
fn owner_attachment_declaration_crosses_registration_wire() {
    mod_api::__private::initialize().unwrap();
    install_counter();
    let _caller = mod_api::content::crash::enter_native_mod(mod_api::content::crash::ModCall {
        id: "b",
        hook: "test",
        part: None,
    });
    assert_eq!(
        mod_api::__private::validate_attachment_use("wire_test", "mana", false),
        Ok(())
    );
}

#[test]
fn named_undeclared_attachment_error_crosses_wire() {
    let error = mod_api::AttachmentError::Undeclared {
        package_id: "test_pkg".into(),
        attachment_id: "test_pkg:mana".into(),
    };
    let mut writer = Writer::new();
    writer.put(&error);
    assert!(writer.error().is_none());
    let mut reader = Reader::new(writer.bytes());
    assert_eq!(reader.take::<mod_api::AttachmentError>().unwrap(), error);
}
