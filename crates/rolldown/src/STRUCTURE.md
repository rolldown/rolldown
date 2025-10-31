# Source Code Structure

This document describes the organization of the `src/` directory for the rolldown bundler core.

## Directory Layout

```
src/
├── bundler/           - Core Bundler implementation
│   ├── bundler.rs     - Main Bundler struct with public API
│   └── builder.rs     - Builder pattern for constructing Bundler instances
│
├── stages/            - Build pipeline stages
│   ├── scan_stage.rs          - Module scanning and parsing
│   ├── link_stage/            - Module linking and tree-shaking
│   │   ├── bind_imports_and_exports.rs
│   │   ├── tree_shaking/      - Tree-shaking implementation
│   │   └── ...
│   └── generate_stage/        - Code generation and chunking
│       ├── chunk_graph.rs     - Graph structure for chunks
│       ├── code_splitting.rs  - Code splitting logic
│       └── ...
│
├── module_loader/     - Module loading system
│   ├── module_loader.rs       - Core module loading logic
│   ├── module_task.rs         - Individual module loading tasks
│   ├── external_module_task.rs
│   └── runtime_module_task.rs
│
├── ast_scanner/       - AST scanning and analysis
│   ├── side_effect_detector/  - Side effect detection
│   ├── dynamic_import.rs      - Dynamic import handling
│   ├── cjs_export_analyzer.rs - CommonJS export analysis
│   └── ...
│
├── module_finalizers/ - Module finalization and transformations
│   ├── finalizer_context.rs   - Context for finalization
│   ├── impl_visit_mut.rs      - AST visitor implementation
│   ├── hmr.rs                 - HMR-specific finalization
│   └── rename.rs              - Symbol renaming
│
├── ecmascript/        - ECMAScript code generation
│   ├── ecma_generator.rs      - Main generator
│   ├── ecma_module_view_factory.rs
│   └── format/                - Output format implementations
│       ├── esm.rs             - ES Module format
│       ├── cjs.rs             - CommonJS format
│       ├── iife.rs            - IIFE format
│       └── umd.rs             - UMD format
│
├── hmr/               - Hot Module Replacement
│   ├── hmr_stage.rs           - HMR build stage
│   ├── hmr_ast_finalizer.rs   - AST transformations for HMR
│   └── hmr_module_task.rs     - HMR module tasks
│
├── watch/             - File watching functionality
│   ├── public_watcher.rs      - Public Watcher API
│   ├── watcher.rs             - Watcher implementation
│   ├── emitter.rs             - Event emitter
│   └── event.rs               - Event types
│
├── dev/               - Development mode features
│   ├── dev_engine.rs          - Development server engine
│   ├── build_driver.rs        - Build orchestration
│   └── build_state_machine/   - Build state management
│
├── types/             - Type definitions and data structures
│   ├── bundle_output.rs       - Bundle output types
│   ├── generator.rs           - Generator types
│   └── ...
│
├── utils/             - Utility functions
│   ├── chunk/                 - Chunk-specific utilities
│   │   ├── finalize_chunks.rs
│   │   ├── deconflict_chunk_symbols.rs
│   │   └── ...
│   ├── parse_to_ecma_ast.rs   - AST parsing
│   ├── render_chunks.rs       - Chunk rendering
│   ├── load_source.rs         - Source loading
│   └── ...
│
├── asset/             - Asset handling
├── css/               - CSS handling
├── runtime/           - JavaScript runtime files
├── lib.rs             - Library entry point
└── type_alias.rs      - Type aliases
```

## Module Organization Principles

### 1. **Bundler** (`bundler/`)

The core bundler implementation is separated from the rest of the codebase. This includes:

- The main `Bundler` struct with the public API
- The builder pattern for constructing bundler instances

### 2. **Build Pipeline Stages** (`stages/`)

The bundling process is organized into distinct stages:

- **Scan Stage**: Parses modules and builds the module graph
- **Link Stage**: Links modules together and performs tree-shaking
- **Generate Stage**: Generates output chunks and assets

The `ChunkGraph` is located in `generate_stage/` as it's primarily used during code generation.

### 3. **Module System** (`module_loader/`, `module_finalizers/`)

Module-related functionality is split into:

- **Module Loader**: Handles loading and resolving modules
- **Module Finalizers**: Performs final transformations on modules (AST manipulation, renaming, etc.)

### 4. **Analysis** (`ast_scanner/`)

AST scanning and analysis is centralized, including:

- Side effect detection
- Import/export analysis
- Dynamic import handling
- CommonJS compatibility analysis

### 5. **Code Generation** (`ecmascript/`)

Code generation is separated by output format (ESM, CJS, IIFE, UMD).

### 6. **Development Features** (`dev/`, `hmr/`, `watch/`)

Development-specific features are grouped:

- **Watch**: File watching and change detection
- **HMR**: Hot module replacement
- **Dev**: Development server and build orchestration

### 7. **Utilities** (`utils/`)

General-purpose utility functions that don't fit into specific feature areas.
Chunk-specific utilities are further organized in `utils/chunk/`.

## Import Conventions

- Public exports are re-exported through `lib.rs`
- Internal modules use `crate::` for imports
- Sub-modules use relative imports (`super::`, `crate::module::`)

## Key Files

- `lib.rs` - Main library entry point, defines public API
- `bundler/bundler.rs` - Core bundler implementation
- `stages/scan_stage.rs` - Entry point for module scanning
- `stages/link_stage/mod.rs` - Entry point for linking
- `stages/generate_stage/mod.rs` - Entry point for code generation
