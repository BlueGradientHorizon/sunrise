

#[cfg(any(test, feature = "test-support"))]
pub mod transfer_lifetime_tests {

use super::*;

use sunrise_mod_runtime::attachments::{
        AttachmentLifetime, AttachmentType, Attachments, HasAttachmentLifetime, Holder,
        index::AttachmentIndex,
    };

pub static VALUE: AttachmentType<i32> = AttachmentType {
        namespace: "test",
        name: "transfer",
        default: || 0,
        codec: None,
        copy_on_death: false,
        sync: false,
    };
}


#[cfg(any(test, feature = "test-support"))]
pub mod credits_respawn_tests {

use super::Dim;

use std::sync::{Arc, atomic::AtomicI32, mpsc};

use sunrise_protocol::{
        buf::Reader,
        packets::{
            configuration::ClientInformation,
            play::{self, Clientbound as C, Serverbound as S},
        },
    };

use sunrise_sim::lan::{
        Joined, NetConfig, Out, PlayerSender,
        guest::{Guest, InboxItem, SPECTATOR},
        hub::HubConfig,
    };

pub fn received_packets(rx: &mpsc::Receiver<Out>) -> impl Iterator<Item = C> + '_ {
        rx.try_iter()
            .flat_map(|out| match out {
                Out::Packet(body) => vec![body],
                Out::Packets(bodies) => bodies,
                Out::Disconnect(_) => Vec::new(),
            })
            .filter_map(|body| {
                let mut reader = Reader::new(&body);
                C::decode(reader.varint().ok()?, reader.rest()).ok()
            })
    }
/// The player's saved point: a respawn anchor in the Nether.
    pub const ANCHOR: [i32; 3] = [10, 100, 10];
pub fn placed_guest(conn: u32, dim: Dim, at: [f64; 3]) -> (Guest, mpsc::Receiver<Out>) {
        let (tx, rx) = mpsc::channel();
        let mut guest = Guest::new(
            Joined {
                conn,
                uuid: 330 + u128::from(conn),
                name: format!("Guest{conn}"),
                properties: Vec::new(),
                info: ClientInformation::new(2),
                brand: "test".into(),
                sender: PlayerSender::new(tx, Arc::default()),
                latency: Arc::new(AtomicI32::new(0)),
                address: "127.0.0.1:25565".parse().unwrap(),
                expected: false,
            },
            conn as i32,
            at,
            2,
        );
        guest.dimension = dim.id().into();
        guest.placed = true;
        guest.loaded = true;
        guest.loading = false;
        guest.load_timeout = 0;
        (guest, rx)
    }
}
