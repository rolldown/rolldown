# rolldown_plugin_register_macro

A procedural macro to automatically generate the `register_hook_usage` method for Rolldown plugins.

## Overview

When implementing a Rolldown plugin, you need to manually declare which hooks your plugin implements via the `register_hook_usage` method. This can be error-prone and tedious to maintain. The `RegisterHook` macro automatically analyzes your plugin implementation and generates this method for you.

## Usage

Simply add the `#[RegisterHook]` attribute to your `impl Plugin` block:

```rust
use std::borrow::Cow;
use rolldown_plugin::{Plugin, RegisterHook, HookUsage};

#[derive(Debug)]
struct MyPlugin;

#[RegisterHook]
impl Plugin for MyPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("my-plugin")
    }

    async fn build_start(
        &self,
        _ctx: &PluginContext,
        _args: &HookBuildStartArgs<'_>
    ) -> HookNoopReturn {
        // Your build_start implementation
        Ok(())
    }

    async fn transform(
        &self,
        _ctx: SharedTransformPluginContext,
        _args: &HookTransformArgs<'_>
    ) -> HookTransformReturn {
        // Your transform implementation
        Ok(None)
    }
}
```

The macro will automatically generate:

```rust
fn register_hook_usage(&self) -> HookUsage {
    HookUsage::BuildStart | HookUsage::Transform
}
```

## How It Works

The macro:

1. Scans all methods in your Plugin implementation
2. Identifies which hook methods are implemented (based on method names)
3. Generates a `register_hook_usage` method that returns the appropriate `HookUsage` flags combined with the `|` operator

## Supported Hooks

The macro recognizes all standard Rolldown plugin hooks:

- `build_start` → `HookUsage::BuildStart`
- `resolve_id` → `HookUsage::ResolveId`
- `resolve_dynamic_import` → `HookUsage::ResolveDynamicImport`
- `load` → `HookUsage::Load`
- `transform` → `HookUsage::Transform`
- `module_parsed` → `HookUsage::ModuleParsed`
- `build_end` → `HookUsage::BuildEnd`
- `render_start` → `HookUsage::RenderStart`
- `render_error` → `HookUsage::RenderError`
- `render_chunk` → `HookUsage::RenderChunk`
- `augment_chunk_hash` → `HookUsage::AugmentChunkHash`
- `generate_bundle` → `HookUsage::GenerateBundle`
- `write_bundle` → `HookUsage::WriteBundle`
- `close_bundle` → `HookUsage::CloseBundle`
- `watch_change` → `HookUsage::WatchChange`
- `close_watcher` → `HookUsage::CloseWatcher`
- `transform_ast` → `HookUsage::TransformAst`
- `banner` → `HookUsage::Banner`
- `footer` → `HookUsage::Footer`
- `intro` → `HookUsage::Intro`
- `outro` → `HookUsage::Outro`

## Benefits

- **Automatic**: No need to manually maintain the `register_hook_usage` method
- **Error-free**: Eliminates the risk of forgetting to add a hook to the registration
- **Maintainable**: When you add or remove hooks, the registration updates automatically
- **Type-safe**: Uses compile-time analysis to ensure correctness

## Example: Migrating an Existing Plugin

Before:

```rust
impl Plugin for MyPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("my-plugin")
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::Load | HookUsage::Transform
    }

    async fn load(...) -> HookLoadReturn {
        // implementation
    }

    async fn transform(...) -> HookTransformReturn {
        // implementation
    }
}
```

After:

```rust
#[RegisterHook]
impl Plugin for MyPlugin {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("my-plugin")
    }

    // register_hook_usage is automatically generated!

    async fn load(...) -> HookLoadReturn {
        // implementation
    }

    async fn transform(...) -> HookTransformReturn {
        // implementation
    }
}
```

## Notes

- The macro only recognizes hook methods by their exact names
- Helper methods like `name()` and `*_meta()` methods are ignored
- If no hooks are implemented, the macro generates `HookUsage::empty()`
