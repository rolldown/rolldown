# Analysis of Issue #2628: Skip `module_parsed` hook if unused

## Issue Summary
The issue proposes skipping the `to_module_info()` call and `set_module_info()` call if the `module_parsed` hook is not used by any plugin.

## Current Implementation (after PR #2625)

In `module_task.rs`, for each module:
```rust
let module_info = Arc::new(module.to_module_info(Some(&raw_import_records)));
self.ctx.plugin_driver.set_module_info(&module.id, Arc::clone(&module_info));
self.ctx.plugin_driver.module_parsed(Arc::clone(&module_info), &module).await?;
```

## Cost Analysis

### `to_module_info()` overhead:
- Clones the entire source code string (`self.ecma_view.source.clone()`)
- Clones and sorts importer arrays
- Clones imported ID arrays  
- Iterates and collects export names

For large modules or projects with many modules, this can add up.

### `set_module_info()` overhead:
- Inserts into a `DashMap` - relatively cheap (just `Arc::clone()` after the initial creation)

### `module_parsed()` overhead:
- Already optimized: if `order_by_module_parsed_meta.is_empty()`, the loop doesn't execute

## Challenges with the Proposed Optimization

### 1. `getModuleInfo` usage is not tracked
The `HookUsage` bitflags only track hook methods, not context methods like `getModuleInfo()` and `getModuleIds()`.

Plugins can call `this.getModuleInfo(id)` from ANY hook, such as:
- `renderStart()`
- `buildEnd()`
- `generateBundle()`
- etc.

The module info cache is needed to serve these calls.

### 2. JS plugin runtime behavior cannot be detected statically
For JavaScript plugins, we cannot determine at plugin registration time whether they will call `getModuleInfo()` at runtime. This means:
- We'd need to conservatively assume ALL JS plugins might use it
- The optimization would only work for projects with NO JS plugins
- Most real-world Vite/Rollup projects have JS plugins

### 3. Rust plugins would need explicit declaration
For Rust plugins, we could add a mechanism to declare `GetModuleInfo` usage in their `register_hook_usage()` implementation, but:
- Adds complexity to the plugin API
- Plugin authors must remember to declare it
- Missing declarations would cause runtime errors

## Feasibility Assessment

### ✅ Technically Possible
The optimization CAN be implemented with these constraints:
1. Add `GetModuleInfo` and `GetModuleIds` to `HookUsage` bitflags
2. Require Rust plugins to explicitly declare usage
3. Conservatively assume ALL JS plugins use `getModuleInfo`
4. Only skip `to_module_info()` when:
   - `order_by_module_parsed_meta.is_empty()` (no module_parsed hooks)
   - AND all plugins are Rust plugins
   - AND no Rust plugin declares `GetModuleInfo` or `GetModuleIds` usage

### ⚠️ Limited Benefit
The optimization would only help in these scenarios:
- Projects with NO plugins at all → rare
- Projects with ONLY Rust plugins that don't use `getModuleInfo` → very rare
- Most projects have JS plugins → no benefit

### ❌ Not Worth the Complexity
The implementation would require:
- Modifying the generator to add new `HookUsage` flags
- Updating all Rust plugins to declare usage
- Adding conditional logic in multiple places
- Maintaining the distinction between JS and Rust plugin behavior
- Documenting the requirement for plugin authors

## Recommendation: CLOSE AS "WON'T FIX"

### Reasons:
1. **Limited real-world benefit**: Most projects use JS plugins, which cannot be optimized
2. **Complexity not justified**: The code complexity and maintenance burden outweighs the marginal performance gain
3. **PR #2625 already optimized**: The overhead is already reduced to mainly just the `to_module_info()` call
4. **API burden**: Requires plugin authors to explicitly declare `getModuleInfo` usage
5. **Runtime vs compile-time**: The fundamental issue is that JS plugin behavior is runtime, not compile-time

### Alternative Approaches:
If performance of `to_module_info()` becomes a bottleneck:
1. **Lazy computation**: Delay creating `ModuleInfo` until actually requested via `getModuleInfo()`
2. **Incremental updates**: Only update changed fields instead of recreating the entire `ModuleInfo`
3. **Caching optimizations**: Use `Arc` and copy-on-write for expensive fields like source code

### Conclusion:
While the optimization is technically feasible, the practical benefit is too limited to justify the implementation complexity. The issue should be closed with an explanation of the trade-offs.
