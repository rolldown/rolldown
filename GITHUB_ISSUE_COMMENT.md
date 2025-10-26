# Summary for GitHub Issue #2628

After thorough analysis, I've determined that this optimization is **technically feasible but not recommended** for implementation.

## Analysis

### Current State (after PR #2625)
The code calls `to_module_info()` and `set_module_info()` for every module, regardless of whether any plugin uses the `module_parsed` hook or calls `getModuleInfo()` from the plugin context.

### The Challenge
The main blocker is that `getModuleInfo()` is a **context method, not a hook**:
- Plugins can call `this.getModuleInfo(id)` from ANY hook (renderStart, buildEnd, etc.)
- For **JS plugins**, we cannot detect this statically - it's runtime behavior
- For **Rust plugins**, we could add explicit usage declaration, but this adds API complexity

### Implementation Constraints
To implement this optimization properly, we would need to:
1. Add `GetModuleInfo` and `GetModuleIds` to `HookUsage` tracking
2. Require Rust plugins to explicitly declare usage in `register_hook_usage()`
3. Conservatively assume ALL JS plugins might use `getModuleInfo`
4. Only skip `to_module_info()` when:
   - No plugin uses `module_parsed` hook
   - AND all plugins are Rust plugins  
   - AND no Rust plugin declares `GetModuleInfo`/`GetModuleIds` usage

### Real-World Impact
- ❌ Most projects use JS plugins → **no benefit**
- ❌ Projects with only Rust plugins are rare
- ✅ Projects with NO plugins → would benefit (but this is uncommon)

### Performance vs Complexity
While `to_module_info()` does have some cost (cloning source code, sorting arrays), the optimization would:
- Only help in very limited scenarios
- Require significant code complexity
- Add maintenance burden
- Place new requirements on plugin authors

## Recommendation

**Close this issue as "won't fix"** because:
1. Limited real-world benefit (most projects won't see any improvement)
2. Implementation complexity not justified by the gains
3. PR #2625 already reduced the overhead significantly
4. Cannot statically detect JS plugin runtime behavior

## Alternative Approaches

If `to_module_info()` performance becomes a real bottleneck in the future, consider:
1. **Lazy computation**: Only create `ModuleInfo` when first requested
2. **Incremental updates**: Update specific fields instead of full recreation
3. **Copy-on-write**: Use `Arc` for expensive fields like source code

---

See full analysis in `ISSUE_2628_ANALYSIS.md` for technical details.
