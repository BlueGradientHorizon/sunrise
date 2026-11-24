# Custom enchantment effect kinds

Implement `Hooks::enchant_effect` to handle custom `minecraft:post_attack` effects
when a melee hit lands: a player hitting a mob or another player, or a mob
hitting a mob or a player with an enchanted weapon in its main hand. The hook
runs on the server for each matching entry, including each mob or player hit by
a sweep.

An entry dispatches when:

- The enchantment's `slots` include the main hand.
- `enchanted` is `attacker` and `affected` is `victim`.
- `effect.type` has a namespace other than `minecraft:`. A bare type name uses
  the `minecraft` namespace.
- The entry's `requirements` pass, judged with the hit's damage type.

The event provides the enchantment ID, effect kind, borrowed `Tag` payload,
enchantment level, the `attacker` and `victim` entities and the enchanted
`weapon` stack the attacker hit with, taken before the hit (it may have broken
since). The victim may have died from the hit. Damage the weapon through the attacker's main hand to charge durability.

Other components and attacker/victim pairs and entries nested inside
`minecraft:all_of` do not dispatch this hook. An unclaimed kind does
nothing. Payloads are converted to retained NBT when definitions load; invalid
payloads refuse the definition rather than failing during a hit.
