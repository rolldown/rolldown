# Plugin adapter — Implementation

> The rationale and principles behind this live in [design.md](./design.md).

## Summary

`crates/rolldown_plugin/src/pluginable.rs` defines a concrete `Pluginable` adapter. It owns the
plugin as `Box<dyn Any + Send + Sync>`, caches its `HookUsage`, and stores a typed function pointer
plus metadata function for every hook. `SharedPluginable` is `Arc<Pluginable>`.

`Pluginable::new` calls `register_hook_usage` once. For every registered hook it installs a generic
adapter that restores the concrete plugin type and boxes that hook's future. For every unregistered
hook it installs a shared default and a shared empty metadata function. Public `call_*` methods only
invoke the selected function pointer.

The `define_hooks!` table is the single source of truth for this plumbing. Each row declares the
hook's usage bit, function signature, metadata method, default implementation, and public dispatch
method; the macro generates the typed table fields and adapters from it.

The concrete-type restoration uses one centralized unsafe cast. Its safety invariant is that the
constructor stores a `T` and only installs hook and metadata adapters instantiated with that same
`T`. A debug assertion checks the invariant during development.

`crates/rolldown_plugin/src/plugin_driver/hook_orders.rs` reads the cached usage directly when it
builds the fixed hook-order lists. There is no separate usage vector and no usage check on active
hook calls.

Plugin construction sites must use `Pluginable::new_shared(plugin)` instead of relying on an
`Arc<T>` to `Arc<dyn Pluginable>` coercion.

## Related

- [design.md](./design.md) — the principles and trade-offs behind this
