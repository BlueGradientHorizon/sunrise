
pub trait GeneralGameExt1<'__sg>: crate::game::SideGame<'__sg> {
    /// The declared type key of general row `id`: the level's own row, or on a client the
    /// one its server shows it.
     fn general_kind(&self, id: u32) -> Option<&'static str> {
        let slot = match self.resident_entities.general.row(id) {
            Some(row) => row.type_slot,
            None => self.side_remote_general_type_slot(id)?,
        };
        Some(crate::game::general::general_type_catalog().key(slot))
    }
}
impl<'__sg, G: crate::game::SideGame<'__sg>> GeneralGameExt1<'__sg> for G {}


#[cfg(any(test, feature = "test-support"))]
pub mod tests {
    use crate::game::{
        client_side::remote_client::general::{RemoteGeneral, tick_remote_general},
        remote::Lerp,
    };
    use indexmap::IndexMap;
    use std::collections::HashMap;
    use sunrise_mod_runtime::attachments::Holder;
    use sunrise_protocol::general_entity::{self, Message};
    use sunrise_protocol::packets::play::{self, Clientbound as C};

    pub static FUEL: mod_api::AttachmentType<i32> = mod_api::AttachmentType {
        namespace: "mymod",
        name: "fuel",
        default: || 0,
        codec: Some(mod_api::AttachmentCodec {
            encode: |n| Ok([("n".into(), sunrise_core::nbt::Tag::Int(*n))].into()),
            decode: |c| match c.get(&"n".into()) {
                Some(sunrise_core::nbt::Tag::Int(n)) => Ok(*n),
                _ => Err(mod_api::AttachmentError::Decode),
            },
        }),
        copy_on_death: false,
        sync: true,
    };

    
    #[cfg(test)]
pub(in crate::game) struct Declare;
    #[cfg(test)]
#[mod_api::__subscriptions]
    impl mod_api::Hooks for Declare {
        fn register(&self, r: &mut dyn mod_api::Registrar, _: &mod_api::RegistrationContext) {
            let types = [
                ("missile", 0.5, Some([1.0, 1.25, 0.0]), false),
                ("package", 2.0, None, true),
            ];
            for (name, height, passenger_attachment, pickable) in types {
                r.declare_general_entity(
                    name,
                    mod_api::GeneralEntityCategory::Misc,
                    0.5,
                    height,
                    passenger_attachment,
                    mod_api::GeneralSavePolicy::Save,
                    true,
                    pickable,
                    (!pickable).then_some(mod_api::PushReaction::IgnoreEntity),
                    8,
                    1,
                    1,
                    &[],
                )
                .unwrap();
            }
        }
    }

    pub fn spawn(id: i32, key: &str, at: [f64; 3]) -> C {
        spawn_facing(id, key, at, 0.0)
    }

    pub fn spawn_facing(id: i32, key: &str, at: [f64; 3], yaw: f32) -> C {
        C::CustomPayload(sunrise_protocol::packets::configuration::CustomPayload {
            channel: general_entity::CHANNEL.into(),
            data: general_entity::encode(&Message::Spawn {
                id,
                uuid: 0x77,
                type_key: key.into(),
                position: at,
                rotation: [yaw, 0.0],
                velocity: [0.0; 3],
            })
            .unwrap(),
        })
    }

    pub fn attachment_message(
        id: u32,
        replace: bool,
        value: Option<i32>,
    ) -> sunrise_protocol::attachment_sync::Message {
        use sunrise_core::nbt::Tag;
        use sunrise_mod_runtime::attachments::AttachmentSync;
        let changes = value.map_or_else(Vec::new, |n| {
            vec![AttachmentSync {
                namespace: "mymod".into(),
                name: "fuel".into(),
                value: Some([("n".into(), Tag::Int(n))].into()),
            }]
        });
        sunrise_protocol::attachment_sync::Message {
            holder: Holder::Entity(id),
            replace,
            changes,
        }
    }

    

    

    

    

    

    
}

use crate::game::side_ops::mobs_rest::MobsRestSideGameExt as _;

use crate::game::native_entity_refs_common::NativeEntityRefsCommonGameExt1 as _;

use crate::game::passengers_common::PassengersCommonGameExt1 as _;
