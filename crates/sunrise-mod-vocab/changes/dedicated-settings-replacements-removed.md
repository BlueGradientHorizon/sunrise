# Unused dedicated-settings replacement functions removed

`sunrise_gameplay::game::ServerGame::native_online_mode`, `ServerGame::native_server_port` and the private `ServerGame::dedicated_lan` they called are removed, with the shared helper `sunrise_gameplay_common::game::native_server::online_mode_of` (#6631). Nothing in the game called them: the `server_settings` hook fragment that read them went in `b8881bd88e6`. They were not typed mod API hooks.

The stable `mod_api` surface and published loops are unchanged. Native mods see the four functions leave `internals.json` (advisory), and with them the three replacement slots SDK staging published for the `ServerGame` methods. Those slots fired only when the unused methods were called, so they never observed or changed a running server's settings. A `[[native.replace]]` or `[[native.mixin]]` on one of them no longer resolves; drop it.
