# What a special item renderer costs a draw

A registered renderer extracts a typed argument and submits baked geometry on each draw. It does
not hash the stack, copy its patch or cache a draw recipe. The cost of any component reads and
custom submission work depends on the renderer. A built-in scene whose models do not use a
registered type pays no extra allocations; `built_in_scenes_cost_nothing_more_without_a_listener`
compares it with and without an unmatched registration.

The `special_item_draw_cost` benchmark times `Game::resolve_item` and `submit_item_unlit` for:

- `built_in_model`: stone's own `minecraft:model`.
- `special_direct_submission`: a registered renderer submitting the same stone geometry from
  a bake-time lease.

It checks that the vertex counts match and reports median, p95 and p99 after warm-up. Run it with
prepared resources on an optimized build:

```sh
SUNRISE_TEST_RESOURCES=$PWD/.local/sunrise-client/26.3 cargo test --profile fast -p sunrise-client --lib -- --ignored --nocapture --exact game::native_special_renderers::cost_tests::special_item_draw_cost
```
