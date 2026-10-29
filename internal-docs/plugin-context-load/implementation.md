# Plugin context module loading

`this.load()` sends a `ModuleLoaderMsg::FetchModule` request to the module loader and waits on the plugin driver's `ContextLoadCompletionManager`. A module task marks that ID complete after parsing and `moduleParsed`, before its result is consumed by the module loader.

Entry `resolveId` hooks run before normal module-loader message processing begins. While entry resolution is pending, `ModuleLoader::resolve_user_defined_entries` therefore pumps `FetchModule` messages and starts their module tasks. Other task results are buffered and consumed by the regular message loop once entry resolution finishes. This keeps `this.load()` safe inside an entry's `resolveId` hook without creating a second message consumer.

A module loaded through `this.load()` can later become an entry. When the resolved entry is registered, `try_spawn_new_task` reuses the existing module and promotes its `ModuleInfo.is_entry` flag. Transform and `moduleParsed` hooks may already have run before that promotion, matching the public API warning that entry status can change after parsing.
