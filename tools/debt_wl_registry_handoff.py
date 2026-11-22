#!/usr/bin/env python3
"""Migrate datapack receiver callers from the global slot to connection ownership."""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / 'crates/sunrise-client'


def patch(path, *edits):
    path = ROOT / path
    text = path.read_text()
    for old, new in edits:
        if old not in text:
            raise ValueError((str(path), old))
        text = text.replace(old, new)
    path.write_text(text)


patch('src/datapack_net.rs',
      ('The set a server last\n//! published and the set a client received are held here, for the lan thread and the game.',
       'The server publishes its set for joining clients; each client configuration owns\n//! the set it received.'),
      ('/// What this process\'s client last received from a server.\nstatic RECEIVED: Held = Mutex::new(None);\n', ''),
      ('''/// The set this process's client received from the server it joined, if the server sent one.
pub fn received() -> Option<Arc<mod_api::DatapackRegistrySet>> {
    held(&RECEIVED)
}

/// Forgets what an earlier join received, as a new join starts.
pub fn clear_received() {
    put(&RECEIVED, None);
    clear_transfer();
}

''', ''),
      ('''/// The client's side of a snapshot: checks it against what the client declared and keeps it as
/// the set its mods read, or says why the join must end. Nothing is kept on failure.
pub fn accept(bytes: &[u8]) -> Result<(), String> {
    let declarations = mod_api::__private::datapack_registry_declarations();
    let received = receive(declarations, received().as_deref(), bytes)?;
    put(&RECEIVED, Some(Arc::new(received)));
    Ok(())
}

''', ''),
      ('pub use transport::{accept_chunk, chunks, clear_transfer, finish_transfer};',
       'pub use transport::{Receiver, chunks};'))

patch('src/datapack_transport.rs',
      ('static ASSEMBLY: Mutex<Assembly> = Mutex::new(Assembly { pending: None });\n', ''),
      ('''pub fn clear_transfer() {
    ASSEMBLY.lock().unwrap_or_else(|e| e.into_inner()).pending = None;
}
pub fn finish_transfer() -> Result<(), String> {
    if ASSEMBLY
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .pending
        .is_some()
    {
        Err("Incomplete datapack registry snapshot at the end of configuration.".into())
    } else {
        Ok(())
    }
}
pub fn accept_chunk(packet: &[u8]) -> Result<(), String> {
    let complete = ASSEMBLY
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(packet)?;
    if let Some(bytes) = complete {
        accept(&bytes)?;
    }
    Ok(())
}''', '''/// A single client configuration's synchronized registries and in-flight snapshot.
#[derive(Default)]
pub struct Receiver {
    assembly: Assembly,
    received: Option<Arc<mod_api::DatapackRegistrySet>>,
}

impl Receiver {
    pub fn received(&self) -> Option<&Arc<mod_api::DatapackRegistrySet>> {
        self.received.as_ref()
    }

    pub(crate) fn finish_transfer(&self) -> Result<(), String> {
        if self.assembly.pending.is_some() {
            Err("Incomplete datapack registry snapshot at the end of configuration.".into())
        } else {
            Ok(())
        }
    }

    pub fn accept_chunk(&mut self, packet: &[u8]) -> Result<(), String> {
        if let Some(bytes) = self.assembly.push(packet)? {
            let declarations = mod_api::__private::datapack_registry_declarations();
            let received = receive(declarations, self.received.as_deref(), &bytes)?;
            self.received = Some(Arc::new(received));
        }
        Ok(())
    }
}'''))

patch('src/net.rs',
      ('    Joined {\n        registries: Arc<Dynamic>,', '    Joined {\n        registries: Arc<Dynamic>,\n        datapacks: crate::datapack_net::Receiver,'),
      ('    Reconfigured(Arc<Dynamic>),', '    Reconfigured {\n        registries: Arc<Dynamic>,\n        datapacks: crate::datapack_net::Receiver,\n    },'),
      ('            Self::Joined {\n                registries,\n                uuid: _,\n                name,\n            }',
       '            Self::Joined { registries, name, .. }'),
      ('Self::Reconfigured(registries) =>', 'Self::Reconfigured { registries, .. } =>'),
      ('let registries = configure(', 'let (registries, datapacks) = configure('),
      ('        registries: Arc::new(registries),\n        uuid: profile.uuid,', '        registries: Arc::new(registries),\n        datapacks,\n        uuid: profile.uuid,'),
      ('Event::Reconfigured(Arc::new(registries))', 'Event::Reconfigured { registries: Arc::new(registries), datapacks }'),
      (') -> Result<Dynamic, Reason> {\n    let mut registries', ') -> Result<(Dynamic, crate::datapack_net::Receiver), Reason> {\n    let mut registries'),
      ('crate::datapack_net::clear_received();', 'let mut datapacks = crate::datapack_net::Receiver::default();'),
      ('crate::datapack_net::accept_chunk(&p.data)', 'datapacks.accept_chunk(&p.data)'),
      ('crate::datapack_net::finish_transfer()', 'datapacks.finish_transfer()'),
      ('return Ok(registries);', 'return Ok((registries, datapacks));'),
      ('.map(|registries| registries.menu_extra_revision)', '.map(|(registries, _)| registries.menu_extra_revision)'))

patch('src/game/network/remote/mod.rs',
      ('    pub(super) heard_level: Option<String>,', '    pub(super) heard_level: Option<String>,\n    pub(in crate::game) datapacks: sunrise_client::datapack_net::Receiver,'),
      ('            heard_level: None,', '            heard_level: None,\n            datapacks: Default::default(),'),
      ('                sunrise_client::datapack_net::accept_chunk(&p.data)?;',
       '                if let Some(remote) = &mut self.remote {\n                    remote.datapacks.accept_chunk(&p.data)?;\n                }'))

patch('src/game/network/remote/raw_ingress.rs',
      ('                    registries,\n                    name,', '                    registries,\n                    datapacks,\n                    name,'),
      ('                    self.set_registries(registries);\n                    self.connection_joined();',
       '                    if let Some(remote) = &mut self.remote {\n                        remote.datapacks = datapacks;\n                    }\n                    self.set_registries(registries);\n                    self.connection_joined();'),
      ('                        remote.viewed.clear();', '                        remote.datapacks = Default::default();\n                        remote.viewed.clear();'),
      ('Event::Reconfigured(registries) =>', 'Event::Reconfigured { registries, datapacks } =>'),
      ('                        r.clear_attachments();', '                        r.datapacks = datapacks;\n                        r.clear_attachments();'))

patch('src/game/modding/native_datapack_refs.rs',
      ('''            if self.client_own_level() != Some(level) {
                self.remote.as_ref()?.constructing(level)?;
            }
            sunrise_client::datapack_net::received()''', '''            let remote = self.remote.as_ref()?;
            if self.client_own_level() != Some(level) {
                remote.constructing(level)?;
            }
            remote.datapacks.received().cloned()'''))

patch('src/game/modding/native_registry_entries.rs',
      ('} else if self.remote.is_some() {\n            // This set has already been filtered by the offer, sync policy and accepted schema.\n            sunrise_client::datapack_net::received().filter(|set| set.registry_available(registry))',
       '} else {\n            // This set has already been filtered by the offer, sync policy and accepted schema.\n            self.remote.as_ref()?.datapacks.received().filter(|set| set.registry_available(registry)).cloned()'),
      ('.cloned()\n        } else {\n            None\n        }', '.cloned()\n        }'))

patch('src/game/world/world_entry.rs',
      ('        let received = sunrise_client::datapack_net::received();\n', ''),
      ('            received.as_deref()', '            self.remote.as_ref().and_then(|remote| remote.datapacks.received()).map(Arc::as_ref)'))

patch('tests/datapack_join.rs',
      ('    datapack_net::clear_received();\n', ''),
      ('fn join(host: &mut Host, session: &Session) {', 'fn join(host: &mut Host, session: &Session) -> datapack_net::Receiver {'),
      ('Event::Joined { .. } => Some(()),', 'Event::Joined { datapacks, .. } => Some(datapacks),'),
      ('        _ => None,\n    });\n}\n\n#[test]', '        _ => None,\n    })\n}\n\n#[test]'),
      ('    join(&mut host, &session);\n    let received = datapack_net::received()',
       '    let datapacks = join(&mut host, &session);\n    let received = datapacks.received()'),
      ('    join(&mut host, &session);\n    let joined = datapack_net::received()',
       '    let mut datapacks = join(&mut host, &session);\n    let joined = datapacks.received()'),
      ('power(&received)', 'power(received)'),
      ('play packet carries with `datapack_net::accept`', 'play packet carries in its configuration receiver'),
      ('datapack_net::accept_chunk(&p.data)', 'datapacks.accept_chunk(&p.data)'),
      ('let next = datapack_net::received()', 'let next = datapacks.received()'),
      ('power(&next)', 'power(next)'))

patch('src/game/world/server_reload/datapack_game_delivery_tests.rs',
      ('fn apply_packets(rx: &std::sync::mpsc::Receiver<Out>) -> usize {',
       'fn apply_packets(rx: &std::sync::mpsc::Receiver<Out>, datapacks: &mut datapack_net::Receiver) -> usize {'),
      ('datapack_net::accept_chunk(&p.data)', 'datapacks.accept_chunk(&p.data)'),
      ('                datapack_net::clear_received();', '                let mut datapacks = datapack_net::Receiver::default();'),
      ('                datapack_net::accept(&bytes).unwrap();',
       '                for chunk in datapack_net::chunks(&bytes).unwrap() {\n                    datapacks.accept_chunk(&chunk).unwrap();\n                }'),
      ('datapack_net::received()', 'datapacks.received()'),
      ('apply_packets(&rx)', 'apply_packets(&rx, &mut datapacks)'),
      ('                let mut datapacks = datapack_net::Receiver::default();\n            });', '            });'))

patch('src/game/modding/native_registry_entries_tests.rs',
      ('                sunrise_client::datapack_net::accept(&bytes).unwrap();',
       '                let receiver = &mut game.remote.as_mut().unwrap().datapacks;\n                for chunk in sunrise_client::datapack_net::chunks(&bytes).unwrap() {\n                    receiver.accept_chunk(&chunk).unwrap();\n                }'),
      ('sunrise_client::datapack_net::clear_received();', 'game.remote.as_mut().unwrap().datapacks = Default::default();'))

patch('src/game/world/server_reload/datapack_host_read_tests.rs',
      ('                sunrise_client::datapack_net::clear_received();\n                sunrise_client::datapack_net::accept(&bytes).unwrap();',
       '                let receiver = &mut game.remote.as_mut().unwrap().datapacks;\n                for chunk in sunrise_client::datapack_net::chunks(&bytes).unwrap() {\n                    receiver.accept_chunk(&chunk).unwrap();\n                }'),
      ('                        // Clearing takes the publication lock and drops its reference to the set.\n                        sunrise_client::datapack_net::clear_received();\n', ''),
      ('                assert_eq!(visits, 1);\n                assert_eq!(\n                    level.datapack_read(game, &client_handle,',
       '                assert_eq!(visits, 1);\n                game.remote.as_mut().unwrap().datapacks = Default::default();\n                assert_eq!(\n                    level.datapack_read(game, &client_handle,'))

patch('src/datapack_transport_tests.rs',
      (Path(ROOT / 'src/datapack_transport_tests.rs').read_text().split('    // The real receiver')[1].split('    struct Large;')[0].join(['    // The real receiver', '']), ''),
      ('        clear_received();\n        accept(&snapshot(1, "old".into()).encode().unwrap()).unwrap();',
       '        let mut receiver = Receiver::default();\n        for chunk in chunks(&snapshot(1, "old".into()).encode().unwrap()).unwrap() {\n            receiver.accept_chunk(&chunk).unwrap();\n        }'),
      ('            accept_chunk(data).unwrap();', '            receiver.accept_chunk(data).unwrap();'),
      ('finish_transfer()', 'receiver.finish_transfer()'),
      ('received().unwrap()', 'receiver.received().unwrap()'),
      ('        clear_received();\n', ''))

patch('src/game/network/remote/client_level_lifecycle_tests.rs',
      ('                datapack_net::clear_received();\n                datapack_net::accept(&snapshot).unwrap();',
       '                for chunk in datapack_net::chunks(&snapshot).unwrap() {\n                    game.remote.as_mut().unwrap().datapacks.accept_chunk(&chunk).unwrap();\n                }'),
      ('                datapack_net::clear_received();\n', ''))
