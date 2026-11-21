//! Main-thread actions of `ClientConfigurationPacketListenerImpl`.
use super::{CommonIo, Dynamic, Event, Reason, cfg, ingress, select_known_packs};
use std::sync::{Arc, Mutex};
use sunrise_protocol::retained::OwnedHeap;

#[derive(Default)]
pub(super) struct Configuration {
    registries: Mutex<Registries>,
}

#[derive(Default)]
struct Registries {
    data: Dynamic,
    dialog_snapshot: Option<Arc<Dynamic>>,
}

pub(super) enum Action {
    Ping(cfg::Ping),
    SelectKnownPacks(cfg::SelectKnownPacks),
    RegistryData(cfg::RegistryData),
    UpdateTags(cfg::UpdateTags),
    ShowDialog(cfg::ShowDialog),
}

impl Configuration {
    pub(super) fn queue(
        self: &Arc<Self>,
        io: &Arc<CommonIo>,
        send: &ingress::Producer<Event>,
        action: Action,
    ) -> Result<(), Reason> {
        send.send_wait(Event::Configuration(Box::new(ConfigurationTask {
            listener: self.clone(),
            io: io.clone(),
            action,
        })))
        .map_err(|error| Reason::plain(error.to_string()))
    }

    pub(super) fn take_registries(&self) -> Dynamic {
        std::mem::take(&mut self.registries.lock().unwrap().data)
    }
}

pub struct ConfigurationTask {
    listener: Arc<Configuration>,
    io: Arc<CommonIo>,
    action: Action,
}

impl ConfigurationTask {
    pub(super) fn owned_bytes(&self) -> usize {
        size_of::<Self>()
            + match &self.action {
                Action::Ping(_) => 0,
                Action::SelectKnownPacks(p) => p.owned_heap_bytes(),
                Action::RegistryData(p) => p.owned_heap_bytes(),
                Action::UpdateTags(p) => p.owned_heap_bytes(),
                Action::ShowDialog(p) => p.owned_heap_bytes(),
            }
    }

    pub fn handle(self) -> Option<Event> {
        if !self.io.accepts_packets() {
            return None;
        }
        let reply = match self.action {
            Action::Ping(p) => cfg::Serverbound::Pong(p),
            Action::SelectKnownPacks(p) => {
                cfg::Serverbound::SelectKnownPacks(cfg::SelectKnownPacks {
                    packs: select_known_packs(p.packs),
                })
            }
            Action::RegistryData(p) => {
                let mut registries = self.listener.registries.lock().unwrap();
                if p.registry == "minecraft:dialog" {
                    registries.dialog_snapshot = None;
                }
                registries.data.add(p);
                return None;
            }
            Action::UpdateTags(p) => {
                let mut registries = self.listener.registries.lock().unwrap();
                if p.registries
                    .iter()
                    .any(|row| row.registry == "minecraft:dialog")
                {
                    registries.dialog_snapshot = None;
                }
                registries.data.add_tags(&p);
                return None;
            }
            Action::ShowDialog(p) => {
                let mut registries = self.listener.registries.lock().unwrap();
                let Registries {
                    data,
                    dialog_snapshot,
                } = &mut *registries;
                let snapshot = dialog_snapshot.get_or_insert_with(|| {
                    // Copy only the dynamic dialog registry and its tags, so
                    // later RegistryData and UpdateTags cannot change its links.
                    let mut snapshot = Dynamic::default();
                    let key = "minecraft:dialog";
                    if let Some(entries) = data.registries.get(key) {
                        snapshot.registries.insert(key.into(), entries.clone());
                    }
                    if let Some(tags) = data.tags.get(key) {
                        snapshot.tags.insert(key.into(), tags.clone());
                    }
                    Arc::new(snapshot)
                });
                return Some(Event::ShowDialog {
                    dialog: p.dialog,
                    registries: snapshot.clone(),
                });
            }
        };
        if let Some(writer) = self.io.writer.lock().unwrap().as_ref() {
            let _ = writer.send(&reply.encode());
        }
        None
    }
}
