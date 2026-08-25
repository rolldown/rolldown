# Plugin adapter — Design & Principles

## Summary

Rust plugins implement the ergonomic, statically dispatched `Plugin` trait, while the plugin
driver needs a heterogeneous collection. The adapter preserves that authoring API but erases each
plugin into a compact runtime representation whose hook targets are selected once. See
[implementation.md](./implementation.md) for the machinery.

## Design principles

1. Active hook calls should perform one indirect dispatch, with no repeated hook-usage or type
   checks.
2. Default hooks should have one shared implementation, rather than one async state machine and
   object-safe wrapper per plugin type.
3. `register_hook_usage` remains the single contract for which hooks an instance supports. Static
   plugins should not need a second associated constant, while instance-dependent JavaScript
   plugins must remain supported.
4. Plugin authors keep return-position `impl Future` methods. Making `Plugin` directly object-safe
   would require every implementation to box its futures and would degrade the Rust API.

The adapter trades a small per-instance function table and one extra allocation for less shipped
code and constant data. Plugin counts are small compared with module counts, so this cost is paid
outside hot module-processing loops.

Hook usage is captured when the adapter is constructed. This is consistent with the plugin driver,
which already builds fixed hook order lists from the registered usage; changing usage after plugin
creation is unsupported.

## Rejected alternatives

- Checking `HookUsage` inside every object-safe hook call shares defaults, but adds work to active
  calls, especially dynamically configured JavaScript plugins.
- A static hook-usage associated constant lets the compiler remove defaults, but duplicates
  `register_hook_usage`, requires annotations on every plugin, and cannot describe dynamic usage.
- Making `Plugin` directly object-safe would simplify erasure at the cost of boxed-future boilerplate
  in every plugin implementation.

## Related

- [implementation.md](./implementation.md) — the machinery that realizes this design
