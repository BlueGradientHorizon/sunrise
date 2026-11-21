#!/usr/bin/env python3
"""Migrate player session text and guest address callers (debt item 18)."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
C = Path("crates/sunrise-client/src")
G = C / "game"
V = Path("crates/sunrise-mod-vocab/src")
changes = {}


def replace(path, before, after, count=1):
    if path not in changes:
        changes[path] = (ROOT / path).read_text()
    found = changes[path].count(before)
    if found != count:
        raise SystemExit(f"{path}: expected {count} matches for {before!r}, found {found}")
    changes[path] = changes[path].replace(before, after)


session = V / "host/player_session.rs"
server = G / "modding/native_server.rs"
for method in ("name", "address", "client_brand"):
    signature = f"fn player_{method}(&self, player: PlayerRef) -> "
    for path in (session, server):
        replace(path, signature + "Option<String>", signature + "Option<&str>")
replace(session, "Result<Option<String>, PlayerWorldRefusal>", "Result<Option<&str>, PlayerWorldRefusal>")
replace(server, "Result<Option<String>, mod_api::PlayerWorldRefusal>", "Result<Option<&str>, mod_api::PlayerWorldRefusal>")
replace(server, "return self.lan_acting_name.clone();", "return self.lan_acting_name.as_deref();")
replace(server, "Some(self.online_player_record(player)?.name.clone())", "Some(&self.online_player_record(player)?.name)")
replace(server, "Some(guest.address.ip().to_string())", "Some(guest.ip_address())")
replace(server, "Some(guest.brand.clone())", "Some(&guest.brand)")
replace(server, "(self.native_acting_player() == Some(player)).then(String::new)", "(self.native_acting_player() == Some(player)).then_some(\"\")", 2)
replace(server, "Ok(hub.nicknames.get(&uuid).cloned())", "Ok(hub.nicknames.get(&uuid).map(String::as_str))")

replace(
    V / "testing.rs",
    "fn player_name(&self, player: PlayerRef) -> Option<String> {\n        Some(self.players.get(player.uuid())?.name.clone())",
    "fn player_name(&self, player: PlayerRef) -> Option<&str> {\n        Some(&self.players.get(player.uuid())?.name)",
)
replace(G / "modding/native_command_reads.rs", "GameProfile::new(player.uuid(), name, properties)", "GameProfile::new(player.uuid(), name.to_owned(), properties)")
replace(G / "modding/native_scoreboard.rs", "ScoreHolder::Player(player) => self.player_name(*player),", "ScoreHolder::Player(player) => self.player_name(*player).map(str::to_owned),")
replace(G / "modding/payload_transport/typed_tests.rs", "let name = owner\n                    .name(host)", "let name: &str = owner\n                    .name(host)")

replace(C / "lan/hub.rs", "g.address ==", "g.address() ==", 2)
replace(C / "lan/hub/block_changes.rs", "g.address ==", "g.address() ==")
replace(G / "network/lan/mod.rs", "g.address,", "g.address(),")
replace(G / "network/lan/mod.rs", "g.address.ip().to_string()", "g.ip_address()")
replace(G / "interface/dedicated/status.rs", "&g.address.ip().to_string()", "g.ip_address()")
replace(G / "interface/dedicated/management.rs", "guest.address.ip().to_string()", "guest.ip_address().to_owned()")
for path, count in [
    (G / "administration/commands/profiles.rs", 1),
    (G / "interface/dedicated/mod.rs", 2),
]:
    replace(path, "g.address.ip().to_string() ==", "g.ip_address() ==", count)
    replace(path, "g.address.ip().to_string()", "g.ip_address().to_owned()")
for path, guest, address, count in [
    (C / "lan/hub.rs", "g", "crate::lan::conn::MEMORY_ADDRESS", 2),
    (C / "lan/hub.rs", "g", '\"127.0.0.1:25565\".parse().unwrap()', 1),
    (G / "interface/dedicated/mod.rs", "guest", '\"0.0.0.0:1\".parse().unwrap()', 1),
    (G / "modding/native_guest_turn_tests.rs", "g", '\"192.0.2.12:25565\".parse().unwrap()', 1),
    (G / "modding/native_guest_turn_tests.rs", "g", '\"[2001:db8::13]:25566\".parse().unwrap()', 1),
]:
    replace(path, f"{guest}.address = {address}", f"{guest}.set_address({address})", count)

guests = G / "modding/native_guest_turn_tests.rs"
replace(guests, ".address(game).as_deref()", ".address(game)", 7)
replace(guests, ".name(game).as_deref()", ".name(game)", 2)
replace(G / "modding/native_player_visibility_tests.rs", 'Ok(Some("Cap".to_owned()))', 'Ok(Some("Cap"))', 3)

for path, source in changes.items():
    (ROOT / path).write_text(source)
