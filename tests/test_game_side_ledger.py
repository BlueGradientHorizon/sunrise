#!/usr/bin/env python3
"""Unit tests for `tools/game_side_ledger.py`."""

from __future__ import annotations

import io
import os
import subprocess
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "tools"))

import game_side_ledger as gsl  # noqa: E402


def _write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def _make_base_repo(tmp: Path, line_pad: int = 0, alt_client_path: bool = False) -> dict[str, Path]:
    """Create a minimal synthetic repo with side tables, Rust sources, and MIR fixtures."""
    pad = "\n" * line_pad
    c_mod_decl = '#[path = "renamed_client_leaf.rs"]\n    pub mod cmod;' if alt_client_path else "pub mod cmod;"
    c_file_rel = "crates/sunrise-client/src/game/renamed_client_leaf.rs" if alt_client_path else "crates/sunrise-client/src/game/client_side/cmod.rs"

    _write(
        tmp / "tools/game_module_sides.tsv",
        "\n".join(
            [
                "module\tside\treason\tpaths",
                f"cmod\tclient\tclient test module\t{c_file_rel}",
                "smod\tserver\tserver test module\tcrates/sunrise-client/src/game/server_side/smod.rs",
                "host\tgame\tcommon host module\tcrates/sunrise-client/src/game/host/mod.rs",
            ]
        )
        + "\n",
    )
    _write(
        tmp / "tools/game_fn_sides.tsv",
        "\n".join(
            [
                "module\timpl\tname\tcfg\tside\treason",
                "host\timpl Game<'_>\tserver_helper\t-\tserver\ttest row",
            ]
        )
        + "\n",
    )
    _write(
        tmp / "tools/game_field_owners.tsv",
        "\n".join(
            [
                "field\ttype\towner\treason\tdeclaration_index\tvisibility\twrapper",
                "res\tu32\tgame\tshared\t0\t-\t-",
                "audio\tAudio\tgame\twrapped client\t1\t-\tclient_shared",
                "friends\tbool\tgame\tcfg field\t2\t-\t-",
                "save\tSave\tgame\twrapped server\t3\t-\tserver_only",
                "screen\tu32\tclient\tclient state\t4\tgame\t-",
                "lan_acting\tu32\tserver\tserver state\t5\tgame\t-",
            ]
        )
        + "\n",
    )

    _write(
        tmp / "crates/sunrise-client/src/game.rs",
        pad
        + f"""pub mod client_side;
pub mod server_side;
pub mod host;

struct Game<'a> {{
    res: u32,
    audio: client_side::ClientShared<Audio>,
    #[cfg(feature = "friends")]
    friends: bool,
    save: server_side::ServerOnly<Save>,
    pub(in crate::game) server: server_side::ServerState,
    pub(in crate::game) client: client_side::ClientState,
}}

impl<'a> Game<'a> {{
    fn run_with_cb<F>(&mut self, cb: F) where F: FnOnce(&mut Self) {{}}
    fn pure_transform(&self, f: impl FnOnce(Text) -> Text) -> Text {{ todo!() }}
    #[cfg(test)]
    fn test_only_cb(&mut self, cb: impl FnOnce(&mut Self)) {{}}
}}

#[cfg(windows)]
fn win_only() {{}}
const FAKE: &str = "#[cfg(fake_in_string)]";
// #[cfg(fake_in_comment)]
""",
    )
    _write(
        tmp / "crates/sunrise-client/src/game/client_side.rs",
        pad
        + f"""{c_mod_decl}
pub(in crate::game) struct ClientState {{
    pub(in crate::game) screen: u32,
}}
pub(in crate::game) struct ClientShared<T>(pub(in crate::game) T);
""",
    )
    _write(
        tmp / "crates/sunrise-client/src/game/server_side.rs",
        pad
        + """pub mod smod;
pub(in crate::game) struct ServerState {
    pub(in crate::game) lan_acting: u32,
}
pub(in crate::game) struct ServerOnly<T>(pub(in crate::game::server_side) T);
""",
    )
    _write(
        tmp / c_file_rel,
        pad
        + """use crate::game::Game;
impl Game<'_> {
    pub fn client_draw(&mut self) {}
}
pub fn client_free() {}
""",
    )
    _write(
        tmp / "crates/sunrise-client/src/game/server_side/smod.rs",
        pad
        + """use crate::game::Game;
pub struct ServerGuard;
impl Game<'_> {
    pub fn server_tick(&mut self) {}
    pub fn with_scope(&mut self, f: impl FnOnce(&mut Self)) {}
}
impl Drop for ServerGuard {
    fn drop(&mut self) {}
}
""",
    )
    _write(
        tmp / "crates/sunrise-client/src/game/host/mod.rs",
        pad
        + """use crate::game::Game;
pub trait Host {
    fn host_call(&mut self);
}
pub struct Holder {
    guard: crate::game::server_side::smod::ServerGuard,
}
impl Host for Game<'_> {
    fn host_call(&mut self) {}
}
impl Game<'_> {
    pub fn common_entry(&mut self, f: fn(&mut Game<'_>), g: &mut dyn FnMut(u32) -> bool) {}
    pub fn dyn_host_cb(&mut self, cb: &mut dyn FnMut(&mut dyn Host)) {}
}
type GameFnAlias = fn(&mut Game<'_>, u32);
pub fn takes_alias(h: GameFnAlias) {}
""",
    )

    # Line numbers matching impl blocks above (shifted by `line_pad`)
    c_impl_line = 2 + line_pad
    s_impl_line = 3 + line_pad
    s_drop_line = 7 + line_pad
    h_trait_line = 8 + line_pad
    h_impl_line = 11 + line_pad

    mir_default = f"""// MIR fixture (default features: friends enabled -> save is idx 3, server is 4, client is 5)
fn game::client_side::cmod::<impl at {c_file_rel}:{c_impl_line}:1: {c_impl_line}:14>::client_draw(_1: &mut game::Game<'_>) -> () {{
    let mut _2: &mut game::Game<'_>;
    let mut _3: u32;
    let mut _4: &mut u32;
    let mut _5: {{closure@{c_file_rel}:{c_impl_line + 1}:5: {c_impl_line + 1}:10}};
    bb0: {{
        // Same-side call (must NOT emit call row)
        _0 = game::client_side::cmod::client_free() -> [return: bb1, unwind continue]; // scope 0 at {c_file_rel}:{c_impl_line + 1}:5: {c_impl_line + 1}:10
    }}
    bb1: {{
        // Cross-side call client -> server
        _0 = game::server_side::smod::<impl game::Game<'_>>::server_tick(copy _1) -> [return: bb2, unwind continue]; // scope 0 at {c_file_rel}:{c_impl_line + 1}:12: {c_impl_line + 1}:20
    }}
    bb2: {{
        // Cross-side field read + borrow-mut + write on ServerState and wrapped ServerOnly
        _3 = copy (((*_1).4: game::server_side::ServerState).0: u32); // scope 0 at {c_file_rel}:{c_impl_line + 1}:21: {c_impl_line + 1}:25
        _4 = &mut (((*_1).4: game::server_side::ServerState).0: u32); // scope 0 at {c_file_rel}:{c_impl_line + 1}:26: {c_impl_line + 1}:30
        ((((*_1).3: game::server_side::ServerOnly<Save>).0: Save)) = move _2; // scope 0 at {c_file_rel}:{c_impl_line + 1}:31: {c_impl_line + 1}:35
        // Pass client-using closure to server function (`closure-pass`)
        _0 = game::server_side::smod::<impl game::Game<'_>>::with_scope(copy _1, move _5) -> [return: bb3, unwind continue]; // scope 0 at {c_file_rel}:{c_impl_line + 1}:36: {c_impl_line + 1}:40
    }}
    bb3: {{
        return;
    }}
}}

fn game::client_side::cmod::<impl at {c_file_rel}:{c_impl_line}:1: {c_impl_line}:14>::client_draw::{{closure#0}}(_1: {{closure@{c_file_rel}:{c_impl_line + 1}:5: {c_impl_line + 1}:10}}, _2: &mut game::Game<'_>) -> () {{
    let mut _3: u32;
    bb0: {{
        _3 = copy (((*_2).5: game::client_side::ClientState).0: u32); // scope 0 at {c_file_rel}:{c_impl_line + 1}:8: {c_impl_line + 1}:12
        return;
    }}
}}

fn game::client_side::cmod::client_free() -> () {{
    bb0: {{
        return;
    }}
}}

fn game::server_side::smod::<impl at crates/sunrise-client/src/game/server_side/smod.rs:{s_impl_line}:1: {s_impl_line}:14>::server_tick(_1: &mut game::Game<'_>) -> () {{
    let mut _2: &mut Audio;
    let mut _3: fn();
    let mut _4: &mut game::client_side::ClientState;
    let mut _5: std::boxed::Box<game::client_side::ClientState>;
    let mut _6: std::ptr::Unique<game::client_side::ClientState>;
    bb0: {{
        // Wrapped ClientShared borrow-mut from server
        _2 = &mut (((*_1).1: game::client_side::ClientShared<Audio>).0: Audio); // scope 0 at crates/sunrise-client/src/game/server_side/smod.rs:{s_impl_line + 1}:5: {s_impl_line + 1}:10
        // Whole ClientState container borrow (`Game.client`)
        _4 = &mut ((*_1).5: game::client_side::ClientState); // scope 0 at crates/sunrise-client/src/game/server_side/smod.rs:{s_impl_line + 1}:7: {s_impl_line + 1}:10
        // Box<ClientState>.0 must NOT be mistaken for ClientState field 0
        _6 = copy (_5.0: std::ptr::Unique<game::client_side::ClientState>); // scope 0 at crates/sunrise-client/src/game/server_side/smod.rs:{s_impl_line + 1}:8: {s_impl_line + 1}:10
        // Reify client fn pointer (`fnptr` form=reify)
        _3 = game::client_side::cmod::client_free as fn() (PointerCoercion(ReifyFnPointer(Safe), Implicit)); // scope 0 at crates/sunrise-client/src/game/server_side/smod.rs:{s_impl_line + 1}:11: {s_impl_line + 1}:15
        // Pass client fn item directly as argument (`fnptr` form=fn-value)
        _0 = std::option::Option::<u32>::map::<(), fn() {{game::client_side::cmod::client_free}}>(const 0_u32, game::client_side::cmod::client_free) -> [return: bb1, unwind continue]; // scope 0 at crates/sunrise-client/src/game/server_side/smod.rs:{s_impl_line + 1}:16: {s_impl_line + 1}:20
    }}
    bb1: {{
        return;
    }}
}}

fn game::server_side::smod::<impl at crates/sunrise-client/src/game/server_side/smod.rs:{s_impl_line}:1: {s_impl_line}:14>::with_scope(_1: &mut game::Game<'_>, _2: F) -> () {{
    bb0: {{
        _0 = <F as std::ops::FnOnce<(&mut game::Game<'_>,)>>::call_once(move _2, copy _1) -> [return: bb1, unwind continue]; // scope 0 at crates/sunrise-client/src/game/server_side/smod.rs:{s_impl_line + 2}:5: {s_impl_line + 2}:10
    }}
    bb1: {{
        return;
    }}
}}

fn game::server_side::smod::<impl at crates/sunrise-client/src/game/server_side/smod.rs:{s_drop_line}:1: {s_drop_line}:26>::drop(_1: &mut game::server_side::smod::ServerGuard) -> () {{
    bb0: {{
        _0 = game::server_side::smod::<impl game::Game<'_>>::server_tick(copy _1) -> [return: bb1, unwind continue]; // scope 0 at crates/sunrise-client/src/game/server_side/smod.rs:{s_drop_line + 1}:5: {s_drop_line + 1}:10
    }}
    bb1: {{
        return;
    }}
}}

fn game::host::<impl at crates/sunrise-client/src/game/host/mod.rs:{h_trait_line}:1: {h_trait_line}:25>::host_call(_1: &mut game::Game<'_>) -> () {{
    let mut _2: &game::host::Holder;
    let mut _3: &game::server_side::smod::ServerGuard;
    bb0: {{
        _3 = &((*_2).0: game::server_side::smod::ServerGuard); // scope 0 at crates/sunrise-client/src/game/host/mod.rs:{h_trait_line + 1}:5: {h_trait_line + 1}:10
        _0 = game::client_side::cmod::<impl game::Game<'_>>::client_draw(copy _1) -> [return: bb1, unwind continue]; // scope 0 at crates/sunrise-client/src/game/host/mod.rs:{h_trait_line + 1}:11: {h_trait_line + 1}:20
    }}
    bb1: {{
        return;
    }}
}}

fn game::host::<impl at crates/sunrise-client/src/game/host/mod.rs:{h_impl_line}:1: {h_impl_line}:14>::common_entry(_1: &mut game::Game<'_>, _2: fn(&mut game::Game<'_>), _3: &mut dyn std::ops::FnMut(u32) -> bool) -> () {{
    let mut _4: bool;
    bb0: {{
        // Indirect fn pointer call (`fnptr`)
        _0 = move _2(copy _1) -> [return: bb1, unwind continue]; // scope 0 at crates/sunrise-client/src/game/host/mod.rs:{h_impl_line + 1}:5: {h_impl_line + 1}:10
    }}
    bb1: {{
        // Dynamic trait call with arrow in signature (`dyn`)
        _4 = <dyn std::ops::FnMut(u32) -> bool as std::ops::FnMut<(u32,)>>::call_mut(copy _3, const 1_u32) -> [return: bb2, unwind continue]; // scope 0 at crates/sunrise-client/src/game/host/mod.rs:{h_impl_line + 1}:12: {h_impl_line + 1}:20
    }}
    bb2: {{
        return;
    }}
}}

fn game::host::ctor_check() -> () {{
    let mut _1: game::Game<'_>;
    let mut _2: game::client_side::ClientState;
    let mut _3: game::server_side::ServerState;
    bb0: {{
        _2 = game::client_side::ClientState {{ screen: const 0_u32 }}; // scope 0 at crates/sunrise-client/src/game/host/mod.rs:{h_impl_line}:1: {h_impl_line}:5
        _3 = game::server_side::ServerState {{ lan_acting: const 0_u32 }}; // scope 0 at crates/sunrise-client/src/game/host/mod.rs:{h_impl_line}:1: {h_impl_line}:5
        _1 = game::Game {{ res: const 0_u32, audio: move _2, friends: const false, save: move _3, server: move _3, client: move _2 }}; // scope 0 at crates/sunrise-client/src/game/host/mod.rs:{h_impl_line}:1: {h_impl_line}:5
        return;
    }}
}}
"""

    # no-default-features: `friends` cfg is off -> `save` shifts to idx 2, `server` is 3, `client` is 4
    mir_nodef = f"""// MIR fixture (no-default-features)
fn game::client_side::cmod::<impl at {c_file_rel}:{c_impl_line}:1: {c_impl_line}:14>::client_draw(_1: &mut game::Game<'_>) -> () {{
    let mut _2: &mut game::Game<'_>;
    bb0: {{
        ((((*_1).2: game::server_side::ServerOnly<Save>).0: Save)) = move _2; // scope 0 at {c_file_rel}:{c_impl_line + 1}:31: {c_impl_line + 1}:35
        return;
    }}
}}

fn game::host::ctor_check() -> () {{
    let mut _1: game::Game<'_>;
    bb0: {{
        _1 = game::Game {{ res: const 0_u32, audio: move _1, save: move _1, server: move _1, client: move _1 }}; // scope 0 at crates/sunrise-client/src/game/host/mod.rs:{h_impl_line}:1: {h_impl_line}:5
        return;
    }}
}}
"""

    dump_dir = tmp / "dumps"
    _write(dump_dir / "default.mir", mir_default)
    _write(dump_dir / "no-default-features.mir", mir_nodef)
    return {
        "root": tmp,
        "dump_dir": dump_dir,
        "table": tmp / "tools/game_side_ledger.tsv",
    }


def _run_cli(args: list[str]) -> tuple[int, str, str]:
    out_buf = io.StringIO()
    err_buf = io.StringIO()
    with redirect_stdout(out_buf), redirect_stderr(err_buf):
        rc = gsl.main(args)
    return rc, out_buf.getvalue(), err_buf.getvalue()


class GameSideLedgerTests(unittest.TestCase):
    def test_all_row_kinds_and_cfg_shift(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            ctx = _make_base_repo(Path(td))
            rc, out, err = _run_cli(
                [
                    "--write",
                    "--root",
                    str(ctx["root"]),
                    "--dump-dir",
                    str(ctx["dump_dir"]),
                    "--configs",
                    "default,no-default-features",
                ]
            )
            self.assertEqual(rc, 0, msg=err)
            rows, _ = gsl.load_ledger_tsv(ctx["table"])
            kinds = {r.kind for r in rows}
            self.assertEqual(
                kinds,
                {
                    "call",
                    "field",
                    "dyn",
                    "generic",
                    "fnptr",
                    "closure-pass",
                    "trait-impl",
                    "drop",
                    "callback-param",
                },
            )

            # Verify no same-side client->client call leaked in
            call_rows = [r for r in rows if r.kind == "call"]
            self.assertFalse(any(r.user_label == "client" and r.target_side == "client" for r in call_rows))
            # Verify client -> server call exists
            self.assertTrue(
                any(
                    r.user_id.endswith("::client_draw")
                    and r.target_id.endswith("::server_tick")
                    and r.target_side == "server"
                    for r in call_rows
                )
            )
            # Verify field modes (read + borrow-mut on ServerState.lan_acting; wrapped Game.save across both configs)
            field_by_target = {(r.user_id, r.target_id): r for r in rows if r.kind == "field"}
            lan_row = next(r for (_, t), r in field_by_target.items() if t == "ServerState.lan_acting")
            self.assertIn("mode=borrow-mut,read", lan_row.detail)
            save_row = next(r for (_, t), r in field_by_target.items() if t == "Game.save")
            self.assertIn("mode=write", save_row.detail)
            self.assertEqual(save_row.configs, "default,no-default-features")
            audio_row = next(r for (_, t), r in field_by_target.items() if t == "Game.audio")
            self.assertIn("mode=borrow-mut", audio_row.detail)
            self.assertIn("storage=client_shared", audio_row.detail)
            # Whole-container borrow `Game.client` recorded; Box<ClientState>.0 NOT mistaken for ClientState.screen
            self.assertTrue(any(t == "Game.client" and "storage=state-container" in r.detail for (_, t), r in field_by_target.items()))
            self.assertFalse(any(u.endswith("::server_tick") and t == "ClientState.screen" for (u, t) in field_by_target))

            # Verify dyn call with `->` in return type parsed cleanly
            dyn_rows = [r for r in rows if r.kind == "dyn"]
            self.assertTrue(any("FnMut" in r.target_id and r.target_id.endswith("::call_mut") for r in dyn_rows))

            # Verify generic call recorded
            gen_rows = [r for r in rows if r.kind == "generic"]
            self.assertTrue(any("FnOnce" in r.target_id and r.target_id.endswith("::call_once") for r in gen_rows))

            # Verify all forms of fnptr (fn-value + reify + indirect-call)
            fnptr_rows = [r for r in rows if r.kind == "fnptr"]
            self.assertTrue(any("fn-value" in r.detail and "reify" in r.detail and r.target_side == "client" for r in fnptr_rows))
            self.assertTrue(any("form=indirect-call" in r.detail and r.target_side == "unresolved" for r in fnptr_rows))

            # Verify closure-pass (client-using closure passed to server::with_scope)
            cp_rows = [r for r in rows if r.kind == "closure-pass"]
            self.assertTrue(any(r.target_id.endswith("::with_scope") and r.target_side == "client" for r in cp_rows))

            # Verify trait-impl in neutral game::host module
            ti_rows = [r for r in rows if r.kind == "trait-impl"]
            self.assertTrue(any(r.user_label == "common" and r.target_id == "Host" for r in ti_rows))

            # Verify drop-impl AND drop-holder
            drop_rows = [r for r in rows if r.kind == "drop"]
            self.assertTrue(any("role=drop-impl" in r.detail and r.target_id.endswith("ServerGuard") for r in drop_rows))
            self.assertTrue(any("role=drop-holder" in r.detail and r.user_id == "game::host::Holder" for r in drop_rows))

            # Verify callback-param captures where-bound, dyn FnMut, fn alias, and skips #[cfg(test)] & pure Text->Text
            cb_users = {r.user_id for r in rows if r.kind == "callback-param"}
            self.assertTrue(any(u.endswith("::run_with_cb") for u in cb_users))
            self.assertTrue(any(u.endswith("::dyn_host_cb") for u in cb_users))
            self.assertTrue(any(u.endswith("::takes_alias") for u in cb_users))
            self.assertFalse(any(u.endswith("::test_only_cb") for u in cb_users))
            self.assertFalse(any(u.endswith("::pure_transform") for u in cb_users))

            # Verify zero `<impl at ...>` spans leaked into any identity
            for r in rows:
                self.assertNotIn("<impl at ", r.user_id)
                self.assertNotIn("<impl at ", r.target_id)

    def test_identity_stability_across_line_move_and_file_rename(self) -> None:
        with tempfile.TemporaryDirectory() as td1, tempfile.TemporaryDirectory() as td2, tempfile.TemporaryDirectory() as td3:
            c1 = _make_base_repo(Path(td1), line_pad=0, alt_client_path=False)
            c2 = _make_base_repo(Path(td2), line_pad=47, alt_client_path=False)
            c3 = _make_base_repo(Path(td3), line_pad=13, alt_client_path=True)

            ids_list = []
            locs_list = []
            for c in (c1, c2, c3):
                rc, _, err = _run_cli(
                    [
                        "--write",
                        "--root",
                        str(c["root"]),
                        "--dump-dir",
                        str(c["dump_dir"]),
                        "--configs",
                        "default,no-default-features",
                    ]
                )
                self.assertEqual(rc, 0, msg=err)
                rows, _ = gsl.load_ledger_tsv(c["table"])
                ids_list.append([r.identity for r in rows])
                locs_list.append({r.identity: r.example_loc for r in rows})

            self.assertEqual(ids_list[0], ids_list[1], "line moves must preserve every row identity")
            self.assertEqual(ids_list[0], ids_list[2], "file rename keeping module path must preserve every row identity")
            self.assertNotEqual(locs_list[0], locs_list[1], "example_loc should reflect moved lines")
            self.assertNotEqual(locs_list[0], locs_list[2], "example_loc should reflect renamed file")

    def test_preserve_hand_columns_and_three_check_failures(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            ctx = _make_base_repo(Path(td))
            base_args = [
                "--root",
                str(ctx["root"]),
                "--dump-dir",
                str(ctx["dump_dir"]),
                "--configs",
                "default,no-default-features",
            ]
            rc, _, err = _run_cli(["--write", *base_args])
            self.assertEqual(rc, 0, msg=err)

            # Failure direction 1: unclassified rows fail without --allow-unclassified
            rc, _, err = _run_cli(["--check", *base_args])
            self.assertEqual(rc, 1)
            self.assertIn("UNCLASSIFIED", err)

            # Passes with --allow-unclassified
            rc, _, err = _run_cli(["--check", "--allow-unclassified", *base_args])
            self.assertEqual(rc, 0, msg=err)

            # Classify every row, re-run --write, verify hand columns are preserved
            rows, _ = gsl.load_ledger_tsv(ctx["table"])
            classified = [
                gsl.LedgerRow(
                    user_id=r.user_id,
                    user_label=r.user_label,
                    kind=r.kind,
                    target_id=r.target_id,
                    target_side=r.target_side,
                    detail=r.detail,
                    configs=r.configs,
                    sites=r.sites,
                    example_loc=r.example_loc,
                    status="bridge",
                    group="G2",
                    note="reviewed note",
                )
                for r in rows
            ]
            ctx["table"].write_text(gsl.format_ledger_tsv(classified), encoding="utf-8")
            rc, out, err = _run_cli(["--write", *base_args])
            self.assertEqual(rc, 0, msg=err)
            self.assertIn("unchanged", out)
            rc, _, err = _run_cli(["--check", *base_args])
            self.assertEqual(rc, 0, msg=err)

            # Failure direction 2: missing row (delete one committed row)
            ctx["table"].write_text(gsl.format_ledger_tsv(classified[1:]), encoding="utf-8")
            rc, _, err = _run_cli(["--check", *base_args])
            self.assertEqual(rc, 1)
            self.assertIn("MISSING", err)

            # Failure direction 3: stale row (add an extra row not in dumps)
            extra = gsl.LedgerRow(
                user_id="game::host::stale_caller",
                user_label="common",
                kind="call",
                target_id="game::client_side::cmod::client_free",
                target_side="client",
                detail="dispatch=free",
                configs="default",
                sites=1,
                example_loc="crates/sunrise-client/src/game/host/mod.rs:1",
                status="debt",
                group="G1",
                note="old",
            )
            ctx["table"].write_text(gsl.format_ledger_tsv(classified + [extra]), encoding="utf-8")
            rc, _, err = _run_cli(["--check", *base_args])
            self.assertEqual(rc, 1)
            self.assertIn("STALE", err)

    def test_against_ratchet_and_missing_baseline(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            ctx = _make_base_repo(root)
            git_env = dict(os.environ, GIT_AUTHOR_NAME="Test", GIT_AUTHOR_EMAIL="t@example.com", GIT_COMMITTER_NAME="Test", GIT_COMMITTER_EMAIL="t@example.com")
            subprocess.run(["git", "init", "-q"], cwd=root, check=True, env=git_env)
            subprocess.run(["git", "add", "tools/game_module_sides.tsv"], cwd=root, check=True, env=git_env)
            subprocess.run(["git", "commit", "-q", "-m", "init without ledger"], cwd=root, check=True, env=git_env)
            early_rev = subprocess.run(["git", "rev-parse", "HEAD"], cwd=root, capture_output=True, text=True, check=True).stdout.strip()

            rc, _, err = _run_cli(
                [
                    "--write",
                    "--root",
                    str(root),
                    "--dump-dir",
                    str(ctx["dump_dir"]),
                    "--configs",
                    "default,no-default-features",
                ]
            )
            self.assertEqual(rc, 0, msg=err)

            # Ref without table fails explicitly unless --allow-missing-baseline is passed
            rc, _, err = _run_cli(["--root", str(root), "--against", early_rev])
            self.assertEqual(rc, 1)
            self.assertIn("--allow-missing-baseline", err)
            rc, _, err = _run_cli(["--root", str(root), "--against", early_rev, "--allow-missing-baseline"])
            self.assertEqual(rc, 0, msg=err)

            # Commit table as baseline
            subprocess.run(["git", "add", "tools/game_side_ledger.tsv"], cwd=root, check=True, env=git_env)
            subprocess.run(["git", "commit", "-q", "-m", "baseline ledger"], cwd=root, check=True, env=git_env)
            base_rev = subprocess.run(["git", "rev-parse", "HEAD"], cwd=root, capture_output=True, text=True, check=True).stdout.strip()

            rows, _ = gsl.load_ledger_tsv(ctx["table"])
            # Removing a row is allowed (ratchet down)
            ctx["table"].write_text(gsl.format_ledger_tsv(rows[1:]), encoding="utf-8")
            rc, _, err = _run_cli(["--root", str(root), "--against", base_rev])
            self.assertEqual(rc, 0, msg=err)

            # Adding a new row fails ratchet
            added = gsl.LedgerRow(
                user_id="game::host::new_caller",
                user_label="common",
                kind="call",
                target_id="game::client_side::cmod::client_free",
                target_side="client",
                detail="dispatch=free",
                configs="default",
                sites=1,
                example_loc="crates/sunrise-client/src/game/host/mod.rs:1",
            )
            ctx["table"].write_text(gsl.format_ledger_tsv(rows + [added]), encoding="utf-8")
            rc, _, err = _run_cli(["--root", str(root), "--against", base_rev])
            self.assertEqual(rc, 1)
            self.assertIn("ADDED vs", err)

    def test_diff_detects_rebound_target(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            t = Path(td)
            old_row = gsl.LedgerRow(
                user_id="game::host::<impl Host for game::Game>::host_call",
                user_label="common",
                kind="call",
                target_id="game::client_side::cmod::<impl game::Game>::items",
                target_side="client",
                detail="dispatch=inherent",
                configs="default",
                sites=1,
                example_loc="crates/sunrise-client/src/game/host/mod.rs:42",
            )
            rebound_row = gsl.LedgerRow(
                user_id="game::host::<impl Host for game::Game>::host_call",
                user_label="common",
                kind="call",
                target_id="game::server_side::smod::<impl ItemsBackend for game::Game>::items",
                target_side="server",
                detail="dispatch=trait:ItemsBackend",
                configs="default",
                sites=1,
                example_loc="crates/sunrise-client/src/game/host/mod.rs:42",
            )
            old_p = t / "old.tsv"
            new_p = t / "new.tsv"
            old_p.write_text(gsl.format_ledger_tsv([old_row]), encoding="utf-8")
            new_p.write_text(gsl.format_ledger_tsv([rebound_row]), encoding="utf-8")

            rc, out, err = _run_cli(["--diff", str(old_p), str(new_p), "--fail-on-rebound"])
            self.assertEqual(rc, 1, msg=err)
            self.assertIn("rebound=1", out)
            self.assertIn("REBOUND (same_loc+symbol)", out)

    def test_refusal_on_unparseable_mir_and_atomic_write(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            ctx = _make_base_repo(Path(td))
            sentinel = "# untouched\n"
            ctx["table"].write_text(sentinel, encoding="utf-8")

            # Unrecognized top-level MIR header
            bad_hdr = Path(td) / "bad_hdr.mir"
            bad_hdr.write_text("mystery_item foo::bar() {}\n", encoding="utf-8")
            rc, _, err = _run_cli(
                [
                    "--write",
                    "--root",
                    str(ctx["root"]),
                    "--dump",
                    f"default={bad_hdr}",
                    "--table",
                    str(Path(td) / "out.tsv"),
                ]
            )
            self.assertEqual(rc, 1)
            self.assertIn("unrecognized top-level MIR header", err)
            self.assertFalse((Path(td) / "out.tsv").exists())

            # Unparsed call terminator
            bad_call = Path(td) / "bad_call.mir"
            bad_call.write_text(
                "fn game::host::broken() -> () {\n    bb0: {\n        weird_term -> [return: bb1];\n    }\n}\n",
                encoding="utf-8",
            )
            rc, _, err = _run_cli(
                [
                    "--write",
                    "--root",
                    str(ctx["root"]),
                    "--dump",
                    f"default={bad_call}",
                    "--table",
                    str(Path(td) / "out2.tsv"),
                ]
            )
            self.assertEqual(rc, 1)
            self.assertIn("unparsed call terminator", err)
            self.assertIn("game::host::broken", err)
            self.assertFalse((Path(td) / "out2.tsv").exists())

            # Constructor order mismatch refuses
            bad_ctor = Path(td) / "bad_ctor.mir"
            bad_ctor.write_text(
                "fn game::host::ctor() -> () {\n    let mut _1: game::client_side::ClientState;\n"
                "    bb0: {\n        _1 = game::client_side::ClientState { wrong_field: const 0_u32 };\n        return;\n    }\n}\n",
                encoding="utf-8",
            )
            rc, _, err = _run_cli(
                [
                    "--write",
                    "--root",
                    str(ctx["root"]),
                    "--dump",
                    f"default={bad_ctor}",
                    "--table",
                    str(Path(td) / "out3.tsv"),
                ]
            )
            self.assertEqual(rc, 1)
            self.assertIn("does not match source", err)

            # Corrupt TSV fails closed without touching existing file
            self.assertEqual(ctx["table"].read_text(encoding="utf-8"), sentinel)
            rc, _, err = _run_cli(
                [
                    "--write",
                    "--root",
                    str(ctx["root"]),
                    "--dump-dir",
                    str(ctx["dump_dir"]),
                    "--configs",
                    "default",
                ]
            )
            self.assertEqual(rc, 1)
            self.assertEqual(ctx["table"].read_text(encoding="utf-8"), sentinel)

    def test_mir_parser_handles_anon_const_char_paren_and_array_ret_type(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            ctx = _make_base_repo(Path(td))
            tricky = Path(td) / "tricky.mir"
            tricky.write_text(
                "\n".join(
                    [
                        "game::host::SCRATCH::{constant#0}: for<'a> fn() -> () = {",
                        "    bb0: {",
                        "        return;",
                        "    }",
                        "}",
                        "alloc42 (fn: <u32 as std::default::Default>::default)",
                        "fn game::host::parse_edge(_1: &str, _2: &dyn std::ops::Fn() -> [u8; 176]) -> () {",
                        "    let mut _3: std::option::Option<&str>;",
                        "    let mut _4: std::option::Option<&dyn std::ops::Fn() -> [u8; 176]>;",
                        "    bb0: {",
                        "        _4 = std::option::Option::<&dyn std::ops::Fn() -> [u8; 176]>::Some(move _2);",
                        "        _3 = core::str::<impl str>::strip_suffix::<char>(copy _1, const ')') -> [return: bb1, unwind continue];",
                        "    }",
                        "    bb1: {",
                        "        return;",
                        "    }",
                        "}",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            sc = gsl.SourceCache(ctx["root"])
            parsed = gsl.parse_mir_file(tricky, ctx["root"], sc)
            self.assertEqual(parsed["skipped_headers"], 0)
            self.assertEqual(parsed["unparsed_call_terms"], 0)
            self.assertEqual(len(parsed["bodies"]), 2)
            self.assertEqual(len(parsed["bodies"][1]["calls"]), 1)

    def test_uncovered_cfgs_ignores_strings_and_comments(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            ctx = _make_base_repo(Path(td))
            uncovered = gsl.scan_uncovered_cfgs(ctx["root"], ("default", "no-default-features"))
            preds = {p for p, _, _ in uncovered}
            self.assertIn("windows", preds)
            self.assertIn("test", preds)
            self.assertNotIn("fake_in_string", preds)
            self.assertNotIn("fake_in_comment", preds)
            self.assertNotIn('feature = "friends"', preds)

    def test_committed_ledger_schema_valid(self) -> None:
        committed = ROOT / "tools/game_side_ledger.tsv"
        if not committed.is_file():
            self.skipTest("tools/game_side_ledger.tsv not generated yet")
        raw = committed.read_text(encoding="utf-8")
        rows, _ = gsl.load_ledger_tsv(committed)
        self.assertGreater(len(rows), 0)
        self.assertEqual(gsl.format_ledger_tsv(rows), raw)
        for r in rows:
            self.assertNotIn("{closure#", r.user_id)
            self.assertNotIn("{constant#", r.user_id)
            self.assertNotIn("promoted[", r.user_id)
            self.assertFalse(r.example_loc.startswith("/"), msg=f"host path leaked in {r.example_loc}")

    def test_real_tree_mir_self_check_when_dumps_provided(self) -> None:
        dump_dir_env = os.environ.get("SUNRISE_LEDGER_DUMP_DIR")
        if not dump_dir_env:
            self.skipTest("set SUNRISE_LEDGER_DUMP_DIR to run real-tree MIR parser self-check")
        dump_dir = Path(dump_dir_env)
        dump_paths = gsl.resolve_dump_paths(dump_dir, [], gsl.DEFAULT_CONFIGS)
        rows, stats = gsl.build_ledger(
            ROOT,
            dump_paths,
            ROOT / "tools/game_module_sides.tsv",
            ROOT / "tools/game_fn_sides.tsv",
            ROOT / "tools/game_field_owners.tsv",
            {},
        )
        for st in stats:
            self.assertEqual(st["skipped_headers"], 0, msg=f"skipped headers in {st}")
            self.assertEqual(st["unparsed_call_terms"], 0, msg=f"unparsed call terms in {st}")
            self.assertEqual(st["missing_local_calls"], 0, msg=f"missing local calls in {st}")
        self.assertGreater(len(rows), 0)


if __name__ == "__main__":
    unittest.main()
