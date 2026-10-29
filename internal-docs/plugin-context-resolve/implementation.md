# Plugin Context Resolve

## Summary

`PluginContextResolveOptions::custom` uses `CustomField`, and `CustomField` is intentionally backed by `TypedMap` instead of `TypedDashMap`.

Today this data is populated before the value is shared and then passed around as `Arc<CustomField>` for read-only access during resolve flows. Concurrent write access is not needed, so the extra sharding and synchronization overhead of `TypedDashMap` would be wasted here.

If plugin-context resolve grows features that need the same custom field to be written from multiple places concurrently, we can revisit this choice and switch `CustomField` back to `TypedDashMap`.

## Related

- `crates/rolldown_plugin/src/types/custom_field.rs`
- `crates/rolldown_plugin/src/types/plugin_context_resolve_options.rs`
