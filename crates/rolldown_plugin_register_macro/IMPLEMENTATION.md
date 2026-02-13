# RegisterHook Macro - Implementation Details

## Overview

The `RegisterHook` macro is a procedural attribute macro that automatically generates the `register_hook_usage` method for Rolldown plugins by analyzing which hook methods are implemented.

## Architecture

### Macro Type

- **Type**: Attribute Macro (Procedural Macro)
- **Target**: `impl Plugin` blocks
- **Output**: Modified impl block with auto-generated `register_hook_usage` method

### Dependencies

```toml
[dependencies]
proc-macro2 = "1"      # Token manipulation
quote = "1"            # Code generation
syn = "2"              # Rust syntax parsing
```

## Implementation Flow

```
┌─────────────────────────────────────────────────────────────┐
│ Input: impl Plugin block with hook methods                 │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│ Parse: Convert TokenStream to syn::ItemImpl                │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│ Analyze: Scan all methods in the impl block                │
│  - Filter for methods matching known hook names            │
│  - Collect matching hook variants                          │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│ Generate: Create register_hook_usage method                │
│  - Empty hooks → HookUsage::empty()                        │
│  - Single hook → HookUsage::HookName                       │
│  - Multiple hooks → HookUsage::Hook1 | HookUsage::Hook2    │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│ Output: Modified impl block with generated method          │
└─────────────────────────────────────────────────────────────┘
```

## Hook Mapping

The macro maintains a static mapping between hook method names and their corresponding `HookUsage` variants:

```rust
let hook_map = vec![
    ("build_start", "BuildStart"),
    ("resolve_id", "ResolveId"),
    ("resolve_dynamic_import", "ResolveDynamicImport"),
    ("load", "Load"),
    ("transform", "Transform"),
    ("module_parsed", "ModuleParsed"),
    ("build_end", "BuildEnd"),
    ("render_start", "RenderStart"),
    ("render_error", "RenderError"),
    ("render_chunk", "RenderChunk"),
    ("augment_chunk_hash", "AugmentChunkHash"),
    ("generate_bundle", "GenerateBundle"),
    ("write_bundle", "WriteBundle"),
    ("close_bundle", "CloseBundle"),
    ("watch_change", "WatchChange"),
    ("close_watcher", "CloseWatcher"),
    ("transform_ast", "TransformAst"),
    ("banner", "Banner"),
    ("footer", "Footer"),
    ("intro", "Intro"),
    ("outro", "Outro"),
];
```

## Code Generation Examples

### Case 1: No Hooks Implemented

**Input:**

```rust
#[RegisterHook]
impl Plugin for EmptyPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("empty")
    }
}
```

**Generated:**

```rust
impl Plugin for EmptyPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("empty")
    }

    fn register_hook_usage(&self) -> rolldown_plugin::HookUsage {
        rolldown_plugin::HookUsage::empty()
    }
}
```

### Case 2: Single Hook

**Input:**

```rust
#[RegisterHook]
impl Plugin for LoadPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("load")
    }

    async fn load(&self, ...) -> HookLoadReturn {
        Ok(None)
    }
}
```

**Generated:**

```rust
impl Plugin for LoadPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("load")
    }

    async fn load(&self, ...) -> HookLoadReturn {
        Ok(None)
    }

    fn register_hook_usage(&self) -> rolldown_plugin::HookUsage {
        rolldown_plugin::HookUsage::Load
    }
}
```

### Case 3: Multiple Hooks

**Input:**

```rust
#[RegisterHook]
impl Plugin for MultiPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("multi")
    }

    async fn build_start(&self, ...) -> HookNoopReturn {
        Ok(())
    }

    async fn transform(&self, ...) -> HookTransformReturn {
        Ok(None)
    }

    async fn generate_bundle(&self, ...) -> HookNoopReturn {
        Ok(())
    }
}
```

**Generated:**

```rust
impl Plugin for MultiPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("multi")
    }

    async fn build_start(&self, ...) -> HookNoopReturn {
        Ok(())
    }

    async fn transform(&self, ...) -> HookTransformReturn {
        Ok(None)
    }

    async fn generate_bundle(&self, ...) -> HookNoopReturn {
        Ok(())
    }

    fn register_hook_usage(&self) -> rolldown_plugin::HookUsage {
        rolldown_plugin::HookUsage::BuildStart
        | rolldown_plugin::HookUsage::Transform
        | rolldown_plugin::HookUsage::GenerateBundle
    }
}
```

## Technical Details

### Token Processing

1. **Parsing**: Uses `syn::parse_macro_input!` to convert the input `TokenStream` into a structured `ItemImpl` AST node.

2. **Method Extraction**: Iterates over `impl.items` and filters for `ImplItem::Fn` variants.

3. **Name Matching**: Compares each method's identifier against the hook map to find implemented hooks.

4. **Code Generation**: Uses `quote!` macro to generate the appropriate Rust code based on the number of detected hooks.

### Fully Qualified Paths

The macro uses fully qualified paths (`rolldown_plugin::HookUsage`) to avoid import issues and ensure the generated code works regardless of the user's imports.

### Bitwise OR Combination

For multiple hooks, the macro uses the `|` operator with Rust's pattern matching in `quote!`:

```rust
quote! {
    #(rolldown_plugin::HookUsage::#hook_idents)|*
}
```

This generates: `HookUsage::A | HookUsage::B | HookUsage::C`

## Testing Strategy

### Unit Tests

Located in `tests/integration_test.rs`:

1. **Multiple Hooks Test**: Verifies correct detection of multiple implemented hooks
2. **Single Hook Test**: Ensures single hooks are handled correctly
3. **Empty Hook Test**: Validates behavior when no hooks are implemented

### Real-World Testing

The macro has been tested on actual Rolldown plugins:

- `rolldown_plugin_oxc_runtime`: Successfully replaced manual registration with macro

## Performance Considerations

### Compile-Time

- All processing happens at compile time
- No runtime overhead
- Generated code is identical to manual implementation

### Code Size

- Generates minimal code (one method)
- No additional dependencies at runtime
- Binary size impact is negligible

## Limitations

1. **Method Name Matching**: Only exact method name matches are detected
   - `build_start` ✅
   - `BuildStart` ❌
   - `build_Start` ❌

2. **Meta Methods Ignored**: Methods ending with `_meta` are not considered hooks
   - `build_start_meta` is ignored

3. **Static Hook List**: New hooks must be added to the macro's hook map

## Future Enhancements

Possible improvements:

1. **Auto-sync with Plugin trait**: Generate hook map from trait definition
2. **Better error messages**: Provide helpful diagnostics for common mistakes
3. **Hook validation**: Warn if a method name looks like a hook but isn't in the map
4. **Performance hints**: Suggest hook ordering optimizations

## Maintenance

### Adding New Hooks

When new hooks are added to the Plugin trait:

1. Add mapping to `hook_map` in `src/lib.rs`
2. Ensure the variant name matches `HookUsage` enum
3. Update documentation and examples
4. Add test cases if needed

### Version Compatibility

The macro depends on:

- `rolldown_plugin` for the Plugin trait and HookUsage type
- Stable Rust features only (no nightly required)
- Procedural macro API (stable since Rust 1.30)

## Comparison with Manual Implementation

### Manual (Before)

```rust
impl Plugin for MyPlugin {
    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::Load | HookUsage::Transform  // Manual maintenance
    }

    async fn load(...) { }      // Easy to forget to update above
    async fn transform(...) { }
}
```

**Problems:**

- Easy to forget updating `register_hook_usage`
- Copy-paste errors
- Maintenance burden

### With Macro (After)

```rust
#[RegisterHook]
impl Plugin for MyPlugin {
    // register_hook_usage generated automatically! ✅

    async fn load(...) { }
    async fn transform(...) { }
}
```

**Benefits:**

- Automatic generation
- Always in sync
- Less code to maintain
- Impossible to forget

## Error Handling

The macro performs basic validation:

- Parses the input as `ItemImpl`
- Handles empty hook lists gracefully
- Uses fully qualified paths to avoid import issues

Compilation errors that may occur:

- Syntax errors in the impl block (caught by rustc)
- Missing imports (unlikely due to fully qualified paths)
- Type mismatches (caught by rustc after macro expansion)
