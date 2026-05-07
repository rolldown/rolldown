
## [1.0.0-rc.18] - 2026-04-29

### 💥 BREAKING CHANGES

- optimization: default unspecified inlineConst.mode to smart (#9248) by @IWANABETHATGUY

### 🐛 Bug Fixes

- rolldown_plugin_vite_import_glob: return error instead of panicking when virtual module uses a relative glob (#9241) by @shulaoda
- binding: treat empty inlineConst object as omitted (#9247) by @IWANABETHATGUY
- rolldown: keep enum declaration for optional-chain access (#9229) by @Dunqing
- link_stage: restore inline let-else in exports-kind filter (#9237) by @IWANABETHATGUY
- dev/lazy: avoid module reinitialization in lazy compilation patches (#9179) by @h-a-n-a
- dev: visit identifier references for runtime rewrites in HMR finalizer (#9191) by @h-a-n-a
- chunk-optimizer: pick dominator for runtime placement to avoid cycles (#9164) by @IWANABETHATGUY
- make `this.emitFile` chunk path synchronous to avoid deadlock (#9031) by @lazarv
- use sentinel id for `browser: false` ignored modules (#9192) by @shulaoda
- prevent chunk optimizer from creating import cycles (#9228) by @IWANABETHATGUY

### 🚜 Refactor

- replace tokio::sync::Mutex with std::sync::Mutex for non-IO data (#9176) by @shulaoda
- rolldown_plugin_vite_import_glob: do not rewrite import path for absolute base (#9195) by @shulaoda
- runtime_helper: wrap DependedRuntimeHelperMap in a struct (#9215) by @IWANABETHATGUY
- drop redundant clear() in determine_safely_merge_cjs_ns (#9206) by @IWANABETHATGUY
- clean up generate_lazy_export (#9208) by @IWANABETHATGUY
- bitset: return bool from set_bit to fuse guard-and-set (#9207) by @IWANABETHATGUY
- link_stage: simplify exports-kind filter and clarify safety comments (#9205) by @IWANABETHATGUY

### 📚 Documentation

- determine_module_exports_kind (#9252) by @IWANABETHATGUY
- fix dead link to esbuild ESM/CJS interop tests (#9230) by @Copilot
- remove CSS bundling references (#9234) by @shulaoda
- correct IncrementalFullBuild row in BundleMode table (#9214) by @IWANABETHATGUY
- design: add bundler data lifecycle design doc (#9212) by @hyf0
- remove minifier alpha status notices (#9202) by @sapphi-red

### ⚙️ Miscellaneous Tasks

- upgrade oxc to 0.128.0 (#9260) by @shulaoda
- deps: bump rolldown-ariadne to 0.6.0 (#9254) by @IWANABETHATGUY
- deps: update github actions (#9259) by @renovate[bot]
- deps: update github actions (#9258) by @renovate[bot]
- remove renovate overrides (#9257) by @Boshen
- use ubuntu-latest for security workflow (#9256) by @Boshen
- notify Discord around release publish (#9251) by @Boshen
- add release environment to npm publish workflow (#9250) by @Boshen
- justfile: drop the `--` separator before forwarded args in `vp run` (#9246) by @shulaoda
- deps: update test262 submodule for tests (#9243) by @sapphi-red
- add more tracing instrumentations (#9220) by @sapphi-red
- rolldown_plugin_vite_import_glob: remove outdated sourcemap doc comment (#9213) by @shulaoda
- update security workflow (#9201) by @Boshen

### ❤️ New Contributors

* @lazarv made their first contribution in [#9031](https://github.com/rolldown/rolldown/pull/9031)


## [1.0.0-rc.17] - 2026-04-22

### 🐛 Bug Fixes

- link: error on missing export between TS modules (#9197) by @IWANABETHATGUY
- rolldown_plugin_vite_import_glob: import path should not be affected by absolute base option (#9145) by @kermanx
- `this.resolve()` returns null for bare relative paths without importer (#9142) by @Copilot
- collect destructured bindings in HMR module exports (#9146) by @h-a-n-a
- esbuild-tests: handle 0.28.0 test cases (#9149) by @sapphi-red
- plugin/copy-module: honor external resolutions from other plugins (#9139) by @TheAlexLichter
- allow undefined in sourcesContent type (#9136) by @jurijzahn8019
- reduce false positives in chunk optimizer circular dependency detection (#9049) by @AlonMiz

### 🚜 Refactor

- chunk-optimizer: extract runtime-module placement into rehome_runtime_module (#9163) by @IWANABETHATGUY

### 📚 Documentation

- add design doc for sort_modules execution ordering (#9169) by @IWANABETHATGUY
- add document for `RenderedModule` (#9147) by @sapphi-red

### ⚡ Performance

- rolldown_plugin_vite_import_glob: skip self-import earlier using raw path comparison (#9193) by @shulaoda

### 🧪 Testing

- lazy: add `playground/lazy-compilation` (#7974) by @hyf0

### ⚙️ Miscellaneous Tasks

- use app token for release PR (#9198) by @Boshen
- upgrade oxc to 0.127.0 (#9194) by @Dunqing
- use oxc security action (#9196) by @Boshen
- esbuild-tests: remove some tests from ignored list as enum inline is now supported (#9184) by @sapphi-red
- deps: update dependency vite-plus to v0.1.19 (#9183) by @renovate[bot]
- use vp instead of pnpm in check-wasi-binding-deps (#9182) by @shulaoda
- verify wasm32-wasi binding deps match @rolldown/browser before publish (#9162) by @shulaoda
- deps: update esbuild for tests to 0.28.0 (#9172) by @sapphi-red
- deps: update rollup submodule for tests to v4.60.2 (#9173) by @sapphi-red
- deps: update test262 submodule for tests (#9174) by @sapphi-red
- sort_modules: fix stale async-entry sort key comment (#9170) by @IWANABETHATGUY
- deps: update npm packages (#9157) by @renovate[bot]
- deps: update dependency diff to v9 (#9158) by @renovate[bot]
- deps: update rust crates (#9156) by @renovate[bot]
- run Windows CI on PRs labeled with `ci: windows` (#9153) by @hyf0
- update-test-dependencies: run setup-rust before file changes (#9151) by @sapphi-red
- deps: update dependency rust to v1.95.0 (#9140) by @renovate[bot]

### ❤️ New Contributors

* @jurijzahn8019 made their first contribution in [#9136](https://github.com/rolldown/rolldown/pull/9136)
* @AlonMiz made their first contribution in [#9049](https://github.com/rolldown/rolldown/pull/9049)


## [1.0.0-rc.16] - 2026-04-16

### 🚀 Features

- const enum cross-module inlining support (#8796) by @Dunqing
- implement module tagging system for code splitting (#9045) by @hyf0

### 🐛 Bug Fixes

- rolldown_plugin_vite_manifest: handle duplicate chunk names for CSS entries (#9059) by @sapphi-red
- improve error message for invalid return values in function options (#9125) by @shulaoda
- await async export-star init wrappers (#9101) by @thezzisu
- never panic during diagnostic emission (#9091) by @IWANABETHATGUY
- include array rest pattern in binding_identifiers (#9112) by @IWANABETHATGUY
- rolldown: set worker thread count with ROLLDOWN_WORKER_THREADS (#9086) by @fpotter
- rolldown_plugin_lazy_compilation: escape request ID in proxy modules (#9102) by @h-a-n-a
- treat namespace member access as side-effect-free (#9099) by @IWANABETHATGUY
- relax overly conservative side-effect leak check in chunk optimizer (#9085) by @IWANABETHATGUY
- runtime: release `cb` reference after `__commonJS` factory initialization (#9067) by @hyf0-agent
- `@__NO_SIDE_EFFECTS__` wrapper should not remove dynamic imports (#9075) by @IWANABETHATGUY
- rolldown_plugin_vite_import_glob: use POSIX path join/normalize for glob resolution (#9077) by @shulaoda
- emit REQUIRE_TLA error when require() loads a module with top-level await (#9071) by @jaehafe
- emit namespace declaration for empty modules in manual chunks (#8993) by @privatenumber
- rolldown_plugin_vite_import_glob: keep common base on path segment boundary (#9070) by @shulaoda
- prevent circular runtime helper imports during facade elimination (#8989) (#9057) by @IWANABETHATGUY
- correct circular dependency check in facade elimination (#9047) by @h-a-n-a
- docs: correct dead link in CodeSplittingGroup.tags JSDoc (#9051) by @hyf0
- emit DUPLICATE_SHEBANG warning when banner contains shebang (#9026) by @IWANABETHATGUY

### 🚜 Refactor

- use semantic reference flags for member write detection (#9060) by @Dunqing
- extract UsedSymbolRefs newtype wrapper (#9130) by @IWANABETHATGUY
- dedupe await wrapping in export-star init emit (#9119) by @IWANABETHATGUY
- calculate side-effect-free function symbols on demand (#9120) by @IWANABETHATGUY
- extract duplicated top-level await handling into shared helper (#9087) by @IWANABETHATGUY
- rolldown_plugin_vite_import_glob: use split_first for get_common_base (#9069) by @shulaoda
- simplify ESM init deduplication with idiomatic insert check (#9044) by @IWANABETHATGUY

### 📚 Documentation

- document runtime module placement strategy in code-splitting design (#9062) by @IWANABETHATGUY
- clarify `options` hook behavior difference with Rollup in watch mode (#9053) by @sapphi-red
- meta/design: introduce module tags (#9017) by @hyf0

### ⚡ Performance

- convert `generate_transitive_esm_init` to iterative (#9046) by @IWANABETHATGUY

### 🧪 Testing

- merge strict/non_strict test variants using configVariants (#9089) by @IWANABETHATGUY

### ⚙️ Miscellaneous Tasks

- disable Renovate auto-updates for oxc packages (#9129) by @IWANABETHATGUY
- upgrade oxc@0.126.0 (#9127) by @Dunqing
- deps: update napi to v3.8.5 (#9126) by @renovate[bot]
- deps: update dependency @napi-rs/cli to v3.6.2 (#9123) by @renovate[bot]
- move lazy-compilation design doc (#9117) by @h-a-n-a
- deps: update dependency vite-plus to v0.1.18 (#9118) by @renovate[bot]
- deps: update dependency vite-plus to v0.1.17 (#9113) by @renovate[bot]
- deps: update oxc to v0.125.0 (#9094) by @renovate[bot]
- deps: update dependency follow-redirects to v1.16.0 [security] (#9103) by @renovate[bot]
- deps: update test262 submodule for tests (#9097) by @sapphi-red
- deps: update crate-ci/typos action to v1.45.1 (#9096) by @renovate[bot]
- deps: update rust crates (#9081) by @renovate[bot]
- deps: update npm packages (#9080) by @renovate[bot]
- remove outdated TODO in determine_module_exports_kind (#9072) by @jaehafe
- rust/test: support `extendedTests: false` shorthand in test config (#9050) by @hyf0
- ci: extract shared infra-changes anchor in path filters (#9054) by @hyf0
- add docs build check to catch dead links in PRs (#9052) by @hyf0

### ❤️ New Contributors

* @thezzisu made their first contribution in [#9101](https://github.com/rolldown/rolldown/pull/9101)
* @fpotter made their first contribution in [#9086](https://github.com/rolldown/rolldown/pull/9086)
* @jaehafe made their first contribution in [#9071](https://github.com/rolldown/rolldown/pull/9071)
* @privatenumber made their first contribution in [#8993](https://github.com/rolldown/rolldown/pull/8993)


## [1.0.0-rc.15] - 2026-04-09

### 🐛 Bug Fixes

- prevent stack overflow in `generate_transitive_esm_init` on circular dependencies (#9041) by @shulaoda

### 🚜 Refactor

- agents: rename Spec-Driven Development to Context Engineering (#9036) by @hyf0


## [1.0.0-rc.14] - 2026-04-08

### 🚀 Features

- rust: add `disable_panic_hook` feature to disable the panic hook (#9023) by @sapphi-red
- support inlineConst for CJS exports accessed through module.exports (#8976) by @h-a-n-a

### 🐛 Bug Fixes

- rolldown_plugin_vite_import_glob: normalize resolved alias path to prevent double slashes (#9032) by @shulaoda
- rolldown_plugin_vite_import_glob: follow symlinks in file scanning (#9000) by @Copilot
- wrap CJS entry modules for IIFE/UMD when using exports/module (#8999) by @IWANABETHATGUY
- emit separate __toESM bindings for mixed ESM/CJS external imports (#8987) by @IWANABETHATGUY
- tree-shake dead dynamic imports to side-effect-free CJS modules (#8529) by @sapphi-red
- skip inlining stale CJS export constants on module.exports reassignment (#8990) by @IWANABETHATGUY

### 🚜 Refactor

- generator: migrate ecma formatting from npx oxfmt to vp fmt (#9022) by @shulaoda
- generator: replace npx oxfmt with vp fmt for ecma formatting (#9021) by @shulaoda

### 📚 Documentation

- contrib-guide: mention that running tests on older Node.js version will have different stat results (#8996) by @Claude

### ⚙️ Miscellaneous Tasks

- deps: update npm packages (#9002) by @renovate[bot]
- deps: update dependency @napi-rs/cli to v3.6.1 (#9034) by @renovate[bot]
- deps: upgrade oxc to 0.124.0 (#9018) by @shulaoda
- deps: update test262 submodule for tests (#9010) by @sapphi-red
- deps: update dependency oxfmt to ^0.44.0 (#9012) by @renovate[bot]
- deps: update dependency vite to v8.0.5 [security] (#9009) by @renovate[bot]
- deps: update dependency vite-plus to v0.1.16 (#9008) by @renovate[bot]
- deps: update rust crates (#9003) by @renovate[bot]
- deps: update github-actions (#9004) by @renovate[bot]
- deps: update dependency lodash-es to v4.18.1 [security] (#8992) by @renovate[bot]
- deps: update crate-ci/typos action to v1.45.0 (#8988) by @renovate[bot]
- upgrade oxc npm packages to 0.123.0 (#8985) by @shulaoda

### ◀️ Revert

- "chore(deps): update dependency oxfmt to ^0.44.0 (#9012)" (#9019) by @shulaoda

### ❤️ New Contributors

* @Claude made their first contribution in [#8996](https://github.com/rolldown/rolldown/pull/8996)


## [1.0.0-rc.13] - 2026-04-01

### 🚀 Features

- add friendly error for unloadable virtual modules (#8955) by @sapphi-red
- better error message for unsupported CSS error (#8911) by @sapphi-red

### 🐛 Bug Fixes

- prevent chunk merging from leaking entry side effects (#8979) by @IWANABETHATGUY
- correct inlining based on module's def format and esModule flag (#8975) by @h-a-n-a
- generate init calls for excluded re-exports in strict execution order (#8858) by @IWANABETHATGUY
- consistent order for `meta.chunks` in `renderChunk` hook (#8956) by @sapphi-red
- subpath imports in glob imports failing to find files (#8885) by @kalvenschraut
- browser: bundle binding types in dts output (#8930) by @nyan-left
- ci: guard artifact download step in `vite-test-ubuntu` when build is skipped (#8934) by @Copilot
- track CJS re-export import records to fix inline const and tree-shaking (#8925) by @h-a-n-a
- use ImportKind::Import for common-chunk root computation (#8899) by @IWANABETHATGUY
- watch: clear emitted_filenames between rebuilds (#8914) by @IWANABETHATGUY
- ci: cache esbuild snapshots to avoid 429 rate limiting (#8921) by @IWANABETHATGUY
- always check circular deps in chunk optimizer (#8915) by @IWANABETHATGUY
- don't mark calls to reassigned bindings as pure (#8917) by @IWANABETHATGUY
- magic-string: throw TypeError for non-string content args (#8905) by @IWANABETHATGUY
- magic-string: add split-point validation and overwrite/update options (#8904) by @IWANABETHATGUY

### 🚜 Refactor

- pre-compute has_side_effects on ChunkCandidate (#8981) by @IWANABETHATGUY
- cleanup and simplify in dynamic_import.rs (#8927) by @ulrichstark
- rename came_from_cjs to came_from_commonjs for consistency (#8938) by @IWANABETHATGUY
- inline `create_ecma_view` return destructuring and remove redundant binding (#8932) by @shulaoda

### 📚 Documentation

- document ensure_lazy_module_initialization_order in code-splitting design doc (#8931) by @IWANABETHATGUY

### 🧪 Testing

- add regression test for runtime helper circular dependency (#8958) by @h-a-n-a
- enable 8 previously-skipped MagicString remove tests (#8945) by @IWANABETHATGUY
- add test for why PureAnnotation is needed in execution order check (#8933) by @IWANABETHATGUY

### ⚙️ Miscellaneous Tasks

- add `@emnapi/runtime` and `@emnapi/core` as direct deps of `@rolldown/browser` (#8978) by @Copilot
- deps: update dependency vite-plus to v0.1.15 (#8970) by @renovate[bot]
- deps: update dependency oxfmt to ^0.43.0 (#8969) by @renovate[bot]
- deps: upgrade oxc to 0.123.0 (#8967) by @shulaoda
- justfile: deduplicate update-submodule as alias of setup-submodule (#8968) by @shulaoda
- deps: update rollup submodule for tests to v4.60.1 (#8965) by @sapphi-red
- deps: update test262 submodule for tests (#8966) by @sapphi-red
- remove unused `type-check` scripts (#8957) by @sapphi-red
- deps: update actions/cache action to v5 (#8953) by @renovate[bot]
- deps: update npm packages to v6 (major) (#8954) by @renovate[bot]
- deps: update npm packages (#8948) by @renovate[bot]
- deps: update rust crates (#8949) by @renovate[bot]
- deps: update github-actions (#8947) by @renovate[bot]
- deps: update napi (#8943) by @renovate[bot]
- deps: update dependency rolldown-plugin-dts to ^0.23.0 (#8944) by @renovate[bot]
- regenerate testing snapshots (#8928) by @ulrichstark
- deps: update dependency rust to v1.94.1 (#8923) by @renovate[bot]

### ❤️ New Contributors

* @kalvenschraut made their first contribution in [#8885](https://github.com/rolldown/rolldown/pull/8885)
* @nyan-left made their first contribution in [#8930](https://github.com/rolldown/rolldown/pull/8930)


## [1.0.0-rc.12] - 2026-03-25

### 🚀 Features

- chunk-optimizer: skip circular dependency check when strict execution order is enabled (#8886) by @hyf0

### 🐛 Bug Fixes

- emit build warnings during watch mode rebuilds (#8897) by @IWANABETHATGUY
- lazy-barrel: load import-then-export specifiers when barrel has local exports (#8895) by @shulaoda
- correct execution order of transferred CJS init calls (#8877) by @IWANABETHATGUY
- mcs: `entriesAware` should calculate sizes without duplication (#8887) by @hyf0
- non-deterministic chunk generation (#8882) by @sapphi-red
- `is_top_level` incorrectly treats strict-mode scopes as top-level (#8878) by @Dunqing

### 🚜 Refactor

- treeshake: migrate SideEffectDetector to Oxc's MayHaveSideEffects trait (#8624) by @Dunqing

### 🧪 Testing

- make dev server tests deterministic by replacing fixed sleeps with event-driven polling (#8561) by @Boshen

### ⚙️ Miscellaneous Tasks

- deps: update dependency vite-plus to v0.1.14 (#8902) by @camc314
- deps: update dependency oxfmt to ^0.42.0 (#8891) by @renovate[bot]
- deps: update rust crate oxc_sourcemap to v6.1.1 (#8890) by @renovate[bot]
- remove Rolldown MF plan (#8883) by @shulaoda
- deps: update rollup submodule for tests to v4.60.0 (#8881) by @sapphi-red
- deps: update test262 submodule for tests (#8880) by @sapphi-red
- deps: upgrade oxc crates to 0.122.0 (#8879) by @shulaoda


## [1.0.0-rc.11] - 2026-03-23

### 🚀 Features

- magicString replace with regex (#8802) by @IWANABETHATGUY
- support `output.sourcemapExcludeSources` option (#8828) by @sapphi-red
- support `getIndentString` in MagicString (#8775) by @IWANABETHATGUY
- MagicString ignoreList support (#8773) by @IWANABETHATGUY

### 🐛 Bug Fixes

- forward test filters through vp run (#8870) by @younggglcy
- types: remove `pluginName` from `MinimalPluginContext` (#8864) by @sapphi-red
- do not report eval?.() as direct eval (#8860) by @IWANABETHATGUY
- handle negative indices, overlapping ranges, and moved content in MagicString remove (#8829) by @IWANABETHATGUY
- enable arbitrary_precision for serde_json to fix JSON float parsing (#8848) by @elderapo
- resolve TypeScript lint errors (#8841) by @Boshen
- avoid panic on multi-byte UTF-8 chars in hash placeholder iterator (#8790) by @shulaoda
- ci: skip failing vite build watch raw query test (#8840) by @Boshen
- ci: use step-level env override to unset VITE_PLUS_CLI_BIN in vite tests (#8838) by @Boshen
- ci: move vite tests into CI workflow by @Boshen
- ci: unset all VITE_PLUS_* env vars in vite-tests workflow (#8837) by @Boshen
- test: skip watch CLI tests on Windows (#8830) by @Boshen
- ci: unset VITE_PLUS_CLI_BIN in vite-tests workflow (#8832) by @Boshen
- remove redundant bare side-effect imports in entry/facade chunks (#8804) by @h-a-n-a
- magicString prepend issues (#8797) by @IWANABETHATGUY
- ci: use `vpx` instead of `vp exec` for `pkg-pr-new` (#8827) by @Boshen
- set `order` for callable plugins (#8815) by @sapphi-red
- handle reversed slice ranges with moved content (#8750) by @IWANABETHATGUY
- update emnapi to latest to avoid version mismatch (#8781) by @sapphi-red
- external.md on Windows OS (#8780) by @bddjr
- align MagicString length/isEmpty with reference magic-string (#8776) by @IWANABETHATGUY

### 🚜 Refactor

- extract canonical_ref_resolving_namespace helper (#8836) by @Boshen

### 📚 Documentation

- improve external examples for cross-platform correctness (#8786) by @hyf0-agent
- update reference to transform function in plugin API documentation (#8778) by @zOadT

### ⚡ Performance

- reduce timing of `dervie_entries_aware_chunk_name` (#8847) by @AliceLanniste
- bench: remove redundant sourcemap benchmark cases (#8825) by @Boshen
- reduce intermediate allocations in `collapse_sourcemaps` (#8821) by @Boshen
- enable parallel AST cloning on macOS (#8814) by @Boshen

### 🧪 Testing

- watch: use polling watcher and retry for watch error test (#8772) by @sapphi-red

### ⚙️ Miscellaneous Tasks

- deps: update dependency @oxc-project/types to v0.122.0 (#8873) by @renovate[bot]
- publish-to-npm: use correct vp pm publish (#8871) by @shulaoda
- justfile: skip setup-vite-plus if vp is already installed (#8862) by @Boshen
- add expectWarning option to test config (#8861) by @IWANABETHATGUY
- justfile: support windows for `just setup` (#8846) by @AliceLanniste
- deps: update rust crates (#8852) by @renovate[bot]
- deps: update endbug/version-check action to v3 (#8855) by @renovate[bot]
- deps: update github-actions (#8853) by @renovate[bot]
- deps: update dependency vitepress to v2.0.0-alpha.17 (#8854) by @renovate[bot]
- deps: update npm packages (#8851) by @renovate[bot]
- bench: use mimalloc as global allocator in bench crate (#8844) by @IWANABETHATGUY
- reuse native build artifact in node-validation job (#8826) by @Boshen
- speed up CodSpeed benchmark build by disabling LTO (#8824) by @Boshen
- remove redundant critcmp benchmark job (#8823) by @Boshen
- deps: update rust crate oxc_sourcemap to v6.1.0 (#8785) by @renovate[bot]
- node: migrate oxlint and oxfmt to Vite+ (#8813) by @Boshen
- revert namespace runners for release build jobs (#8820) by @Boshen
- migrate runners to namespace (#8819) by @Boshen
- test: relax test utils path assertion to support git worktrees (#8816) by @younggglcy
- rename `examples/lazy` to `examples/lazy-compilation` (#8789) by @shulaoda
- improve "needs reproduction" wording by @Boshen
- deps: update dependency oxlint-tsgolint to v0.17.1 (#8807) by @renovate[bot]
- enable 7 previously-skipped MagicString tests (#8771) by @IWANABETHATGUY
- upgrade oxc to 0.121.0 (#8784) by @shulaoda
- increase Windows dev drive size from 12GB to 20GB (#8779) by @Copilot

### ❤️ New Contributors

* @younggglcy made their first contribution in [#8870](https://github.com/rolldown/rolldown/pull/8870)
* @elderapo made their first contribution in [#8848](https://github.com/rolldown/rolldown/pull/8848)
* @bddjr made their first contribution in [#8780](https://github.com/rolldown/rolldown/pull/8780)
* @zOadT made their first contribution in [#8778](https://github.com/rolldown/rolldown/pull/8778)


## [1.0.0-rc.10] - 2026-03-18

### 🚀 Features

- add indentExclusionRanges property to MagicString (#8746) by @IWANABETHATGUY
- expose `oxcRuntimePlugin` (#8654) by @sapphi-red
- rust: make bundler generic over FileSystem for in-memory benchmarks (#8652) by @Boshen

### 🐛 Bug Fixes

- rolldown_plugin_vite_dynamic_import_vars: align dynamic import fast check with Vite (#8760) by @shulaoda
- renamer: handle existing bindings in nested scopes when finding unique names (#8741) by @drewolson
- pass `yarn_pnp` option where needed (#8736) by @sapphi-red
- preserve optional chaining in namespace member expr rewrite (#8712) by @Copilot
- correct UTF-16 index handling in native MagicString (#8693) by @IWANABETHATGUY
- mark failing doctests as ignore (#8700) by @Boshen
- prevent may_partial_namespace from leaking through include_module (#8682) by @IWANABETHATGUY
- ci: bump native-build cache key to invalidate stale napi-rs artifacts (#8678) by @Boshen
- `comments.annotation: false` breaking tree-shaking (#8657) by @IWANABETHATGUY
- validate filenames for NUL bytes from chunkFileNames/entryFileNames (#8644) by @IWANABETHATGUY
- dce-only minify should not set NODE_ENV to production (#8651) by @IWANABETHATGUY

### 🚜 Refactor

- rust: remove dead `CrossModuleOptimizationConfig::side_effects_free_function_optimization` (#8673) by @Dunqing
- rust: simplify `cross_module_optimization` by removing redundant scope tracking (#8672) by @Dunqing
- simplify string repeat in guess_indentor (#8753) by @IWANABETHATGUY
- consolidate custom magic-string tests into one file (#8696) by @IWANABETHATGUY
- extract CJS bailout checks from include_symbol (#8683) by @IWANABETHATGUY
- rust: remove `BindingIdentifierExt` to use `BindingIdentifier::symbol_id()` instead (#8667) by @Dunqing
- bench: add bench_preset helper and inline presets (#8658) by @Boshen
- rust: filter external modules from entries instead of mapping bit positions (#8637) by @Dunqing

### 📚 Documentation

- clarify watch mode behavior and its limitations (#8751) by @sapphi-red
- add external link icon to GitHub button in Hero section (#8731) by @thisisnkc
- guide: clarify that `inject` option is only conceptually similar to esbuild's one (#8743) by @sapphi-red
- meta/design: add `devtools.md` (#8663) by @hyf0
- add viteplus alpha announcement banner (#8668) by @shulaoda

### ⚡ Performance

- rolldown: some minor perf optimization found by autoresearch (#8730) by @Brooooooklyn
- replace Vec allocation with lazy iterator in find_hash_placeholders (#8703) by @Boshen
- replace TypedDashMap with TypedMap in CustomField (#8708) by @Boshen
- bench: remove scan benchmark binary to halve LTO link time (#8694) by @Boshen

### 🧪 Testing

- watch: increase timeout for error output (#8766) by @sapphi-red
- vite-tests: remove JS plugin tests (#8767) by @sapphi-red
- watch: add CLI exit code test (#8752) by @sapphi-red
- normalize paths on Windows even if `resolve.symlinks` is false (#8483) by @sapphi-red

### ⚙️ Miscellaneous Tasks

- correct comment in bundle-analyzer-plugin.ts (#8770) by @origami-z
- upgrade oxc to 0.120.0 (#8764) by @Boshen
- enable all test for `reset` category in MagicString.test.ts (#8749) by @IWANABETHATGUY
- deps: update test262 submodule for tests (#8742) by @sapphi-red
- deps: update oxc apps (#8734) by @renovate[bot]
- deps: update softprops/action-gh-release action to v2.6.1 (#8724) by @renovate[bot]
- deps: update npm packages (major) (#8722) by @renovate[bot]
- deps: update github-actions (major) (#8721) by @renovate[bot]
- deps: update softprops/action-gh-release action to v2.6.0 (#8720) by @renovate[bot]
- deps: update npm packages (#8718) by @renovate[bot]
- deps: update rust crates (#8717) by @renovate[bot]
- deps: update github-actions (#8716) by @renovate[bot]
- deps: update dependency oxlint-tsgolint to v0.17.0 (#8713) by @renovate[bot]
- deps: bump cargo-shear to v1.11.2 (#8711) by @Boshen
- use org level `CODE_OF_CONDUCT.md` (#8706) by @sapphi-red
- fix cache key mismatch and remove redundant cache saves (#8695) by @Boshen
- deps: update oxc apps (#8692) by @renovate[bot]
- deps: update oxc apps (#8649) by @renovate[bot]
- should do matrix out side of reusable workflows 2 (#8691) by @hyf0
- should do matrix out side of reusable workflows (#8690) by @hyf0
- deps: update dependency rolldown-plugin-dts to v0.22.5 (#8689) by @renovate[bot]
- upgrade oxc to 0.119.0 and oxc_resolver to 11.19.1 (#8686) by @Boshen
- correct if condition of `type-check` job (#8677) by @hyf0
- Gate CI type-check job on node changes (#8669) by @Copilot
- benchmark: improve codspeed build (#8665) by @Boshen
- deps: update oxc to v0.118.0 (#8650) by @renovate[bot]
- deps: update crate-ci/typos action to v1.44.0 (#8647) by @renovate[bot]
- deps: update oxc resolver to v11.19.1 (#8646) by @renovate[bot]
- deps: update dependency rust to v1.94.0 (#8648) by @renovate[bot]
- deps: update dependency rolldown-plugin-dts to v0.22.4 (#8645) by @renovate[bot]

### ◀️ Revert

- Revert "ci: Gate CI type-check job on node changes" (#8674) by @hyf0
- "chore(deps): update dependency rust to v1.94.0 (#8648)" (#8660) by @shulaoda

### ❤️ New Contributors

* @origami-z made their first contribution in [#8770](https://github.com/rolldown/rolldown/pull/8770)
* @drewolson made their first contribution in [#8741](https://github.com/rolldown/rolldown/pull/8741)
* @thisisnkc made their first contribution in [#8731](https://github.com/rolldown/rolldown/pull/8731)


## [1.0.0-rc.9] - 2026-03-11

### 💥 BREAKING CHANGES

- rename exported BindingMagicString to RolldownMagicString (#8626) by @IWANABETHATGUY

### 🚀 Features

- rolldown: add isRolldownMagicString property for reliable native detection (#8614) by @IWANABETHATGUY
- cli: align object type with rollup (#8598) by @h-a-n-a

### 🐛 Bug Fixes

- rust: circular inter-chunk imports when external dynamic imports exist (#8596) by @Dunqing
- update minify default docs from `false` to `'dce-only'` (#8620) by @shulaoda

### 💼 Other

- fix early exit in script build-node (#8617) by @h-a-n-a

### 🚜 Refactor

- binding: remove outdated TODO comment in MagicString to_string() (#8613) by @IWANABETHATGUY

### 📚 Documentation

- add viteplus alpha announcement banner (#8615) by @mdong1909
- update VitePress theme to 4.8.2 for narrow-screen layout regression (#8612) by @Copilot

### ⚡ Performance

- merge 4 integration test binaries into 1 (#8610) by @Boshen

### 🧪 Testing

- replace heavy filename_with_hash test with targeted hash fixtures (#8597) by @Boshen

### ⚙️ Miscellaneous Tasks

- ci: remove redundant `--no-run` build step from cargo-test (#8623) by @Boshen
- rust: use `cargo-shear` to toggle Cargo.toml [lib] test = bool (#8622) by @Boshen
- deps: update test262 submodule for tests (#8611) by @sapphi-red
- skip macOS CI jobs on pull requests (#8608) by @Copilot
- add rust cache to repo validation job (#8607) by @Boshen
- skip running empty bin test targets (#8605) by @Boshen
- skip building examples in cargo-test to reduce build time (#8603) by @Boshen
- switch plain workflow checkouts to taiki-e action (#8601) by @Boshen
- skip Windows CI jobs on PRs (#8600) by @Boshen
- remove unused asset module (#8594) by @shulaoda

### ◀️ Revert

- "docs: add viteplus alpha announcement banner (#8615)" (#8616) by @shulaoda


## [1.0.0-rc.8] - 2026-03-09

### 🚀 Features

- watch: enable full functional fs watcher in wasm (#8575) by @hyf0
- watch: expose debounce related options (#8572) by @hyf0

### 🐛 Bug Fixes

- detect new URL(…, import.meta.url) with no-sub template literal (#8565) by @char
- devtools: trace dynamic imports in devtools (#8581) by @cal-gooo
- watch: rebuild when a previously missing file is created (#8562) by @hyf0-agent
- watch: filter out Access events to prevent infinite rebuild loop on Linux (#8557) by @hyf0-agent

### 🚜 Refactor

- watch: remove auto watch for fail imports (#8585) by @hyf0
- fs_watcher: unify the way of constructing watcher (#8571) by @hyf0
- cli: migrate CLI to CAC (#8551) by @h-a-n-a
- switch asset module support from hard-code to builtin plugin (#8546) by @hyf0

### 📚 Documentation

- fix subject-verb agreement in why-bundlers.md (#8591) by @brandonzylstra
- maintenance: align release and canary workflow guide (#8538) by @minsoo-web
- add `format` option to directives example config (#8590) by @shulaoda
- fix: change twitter to x logo in team (#8552) by @mdong1909
- correct composable filter support explanation (#8550) by @sapphi-red

### ⚡ Performance

- testing: share tokio runtime across fixture tests (#8567) by @Boshen

### 🧪 Testing

- hmr: fix infinite loop in dev server test retry logic (#8576) by @hyf0-agent
- cli: add more cli-e2e test cases (#8548) by @h-a-n-a

### ⚙️ Miscellaneous Tasks

- docs: update in-depth/directives for `output.strict` option (#8535) by @minsoo-web
- add PNPM_HOME Dev Drive mapping to Windows CI workflows (#8589) by @Boshen
- deps: update github-actions (#8588) by @renovate[bot]
- move Windows cargo target dir to Dev Drive (#8586) by @Boshen
- optimize cache keys to fix race conditions and reduce usage (#8578) by @Boshen
- remove WASI build & test pipeline (#8580) by @Boshen
- remove unnecessary submodule checkouts (#8577) by @Boshen
- use Dev Drive for Windows CI jobs (#8574) by @Boshen
- skip redundant native binding build for browser and remove standalone job (#8573) by @Boshen
- parallelize Node tests on ubuntu, single Node 24 on macOS/windows (#8570) by @Boshen
- docs: bump @voidzero-dev/vitepress-theme to 4.8.0 (#8558) by @crusty-voidzero
- dedupe type-check from dev server workflow (#8554) by @Boshen

### ❤️ New Contributors

* @brandonzylstra made their first contribution in [#8591](https://github.com/rolldown/rolldown/pull/8591)
* @char made their first contribution in [#8565](https://github.com/rolldown/rolldown/pull/8565)
* @cal-gooo made their first contribution in [#8581](https://github.com/rolldown/rolldown/pull/8581)
* @hyf0-agent made their first contribution in [#8562](https://github.com/rolldown/rolldown/pull/8562)
* @h-a-n-a made their first contribution in [#8551](https://github.com/rolldown/rolldown/pull/8551)


## [1.0.0-rc.7] - 2026-03-05

### 💥 BREAKING CHANGES

- enable minify: 'dce-only' by default (#8465) by @IWANABETHATGUY
- settings `inlineConst: { mode: 'smart', pass: 1}`  by default (#8444) by @IWANABETHATGUY

### 🚀 Features

- binding: add original getter to BindingMagicString (#8533) by @IWANABETHATGUY
- native-magic-string: add `offset` property support (#8531) by @IWANABETHATGUY
- add `output.strict` option to control `"use strict"` directive emission (#8489) by @Copilot
- watch: expose `watcher.compareContentsForPolling` (#8526) by @hyf0
- watch: use new watcher to support watch mode (#8475) by @hyf0
- rust/watch: handle bulk-change (#8466) by @hyf0
- add LLM-friendly markdown output format to bundle analyzer plugin (#8242) by @IWANABETHATGUY

### 🐛 Bug Fixes

- expose `plugins` on `NormalizedInputOptions` for `buildStart` hook (#8521) by @Copilot
- only uppercase facade symbols in JSX preserve mode (#8519) by @IWANABETHATGUY
- binding: export BindingResult in generated dts header (#8537) by @minsoo-web
- pre-resolve paths option to avoid `invoke_sync` deadlock (#8518) by @IWANABETHATGUY
- remove debug-only jsx_preset and UntranspiledSyntaxError (#8511) by @IWANABETHATGUY
- apply `topLevelVar` to exported `const`/`let` declarations (#8507) by @IWANABETHATGUY
- rolldown_plugin_vite_web_worker_post: avoid replacing `new.target` (#8488) by @sapphi-red
- update copyright year to 2026 (#8486) by @maciekzygmunt

### 🚜 Refactor

- rust: use Oxc's SymbolFlags::ConstVariable instead of custom IsConst flag (#8543) by @Dunqing
- rust: remove FacadeScoping, use Scoping::create_symbol for facade symbols (#8540) by @Dunqing
- rust/watch: remove hacky `reset_closed_for_watch_mode` (#8530) by @hyf0
- binding: return &str instead of String in filename() getter (#8534) by @IWANABETHATGUY
- rust: remove old watch mode implementation (#8525) by @hyf0
- rust/watch: simply watch logic in the binding layer (#8516) by @hyf0
- rust/watch: tweak struct/function names (#8464) by @hyf0

### 📚 Documentation

- explain how external modules work in rolldown (#8457) by @sapphi-red
- add some diagrams using graphviz (#8499) by @sapphi-red
- use `vitepress-plugin-graphviz` (#8498) by @sapphi-red
- list s390x/ppc64le prebuilt binaries (#8495) by @crusty-voidzero
- fix error type for `RolldownBuild.generate` and others (#8490) by @sapphi-red

### ⚡ Performance

- string_wizard: reduce allocations and add ASCII fast paths (#8541) by @IWANABETHATGUY
- use IndexBitSet to replace IndexVec<XXXIdx, bool> for module/stmt inclusion tracking (#8503) by @IWANABETHATGUY
- plugin: use IndexBitSet to optimize skipped plugins checking (#8497) by @ShroXd
- rust/tla: skip compute_tla if there is no module use TLA (#8487) by @ShroXd

### 🧪 Testing

- node/watch: make watch tests run in concurrent and retry-able (#8512) by @hyf0
- add test case for static flag tree-shaking (#8476) by @IWANABETHATGUY
- migrate post-banner sourcemap-with-shebang to Rust (#8477) by @Copilot

### ⚙️ Miscellaneous Tasks

- vscode: `formatOnSave` for markdown files using oxc formatter (#8536) by @minsoo-web
- deps: update test262 submodule for tests (#8528) by @sapphi-red
- remove `retry` workaround from output paths test fixtures (#8520) by @Copilot
- docs: add Shuyuan Wang (h-a-n-a) and remove from acknowledgements (#8509) by @Copilot
- consolidate top_level_var test cases using configVariants (#8508) by @IWANABETHATGUY
- add s390x and ppc64le linux gnu targets (#8493) by @Brooooooklyn

### ◀️ Revert

- fix(rolldown): increase tokio blocking threads size for watch mode (#8517) by @hyf0

### ❤️ New Contributors

* @minsoo-web made their first contribution in [#8536](https://github.com/rolldown/rolldown/pull/8536)
* @crusty-voidzero made their first contribution in [#8495](https://github.com/rolldown/rolldown/pull/8495)
* @maciekzygmunt made their first contribution in [#8486](https://github.com/rolldown/rolldown/pull/8486)


## [1.0.0-rc.6] - 2026-02-26

### 💥 BREAKING CHANGES

- css: remove `css_entry_filenames` , `css_chunk_filenames` and related code (#8402) by @hyf0
- css: drop builtin CSS bundling to explore alternative solutions (#8399) by @hyf0

### 🚀 Features

- rust/data-url: use hash as id for data url modules to prevent long string overhead (#8420) by @hyf0
- validate bundle stays within output dir (#8441) by @sapphi-red
- rust: support `PluginOrder::PinPost` (#8417) by @hyf0
- support `ModuleType:Copy` (#8407) by @hyf0
- expose `ESTree` types from `rolldown/utils` (#8400) by @sapphi-red

### 🐛 Bug Fixes

- incorrect sourcemap when postBanner/postFooter is used with shebang (#8459) by @Copilot
- resolver: disable node_path option to align ESM resolver behavior (#8472) by @sapphi-red
- parse `.js` within `"type": "commonjs"` as ESM for now (#8470) by @sapphi-red
- case-insensitive filename conflict detection for chunk deduplication (#8458) by @Copilot
- prevent inlining CJS exports that are mutated by importers (#8456) by @IWANABETHATGUY
- parse `.cjs` / `.cts` / `.js` within `"type": "commonjs"` as CommonJS (#8455) by @sapphi-red
- plugin/copy-module: correct hooks' priority (#8423) by @hyf0
- plugin/chunk-import-map: ensure `render_chunk_meta` run after users plugin (#8422) by @hyf0
- rust: correct hooks order of `DataUriPlugin` (#8418) by @hyf0
- `jsx.preserve` should also considering tsconfig json preserve (#8324) by @IWANABETHATGUY
- `deferred_scan_data.rs "Should have resolved id: NotFound"` error (#8379) by @sapphi-red
- cli: require value for `--dir`/`-d` and `--file`/`-o` (#8378) by @Copilot
- dev: avoid mutex deadlock caused by inconsistent lock order (#8370) by @sapphi-red

### 🚜 Refactor

- watch: rename TaskStart/TaskEnd to BundleStart/BundleEnd (#8463) by @hyf0
- rust: rename `rolldown_plugin_data_uri` to `rolldown_plugin_data_url` (#8421) by @hyf0
- bindingify-build-hook: extract helper for PluginContextImpl (#8438) by @ShroXd
- give source loading a proper name (#8436) by @IWANABETHATGUY
- ban holding DashMap refs across awaits (#8362) by @sapphi-red

### 📚 Documentation

- add glob pattern usage example to input option (#8469) by @IWANABETHATGUY
- remove `https://rolldown.rs` from links in reference docs (#8454) by @sapphi-red
- mention execution order issue in `output.codeSplitting` docs (#8452) by @sapphi-red
- clarify `output.comments` behavior a bit (#8451) by @sapphi-red
- replace npmjs package links with npmx.dev (#8439) by @Boshen
- reference: add `Exported from` for values / types exported from subpath exports (#8394) by @sapphi-red
- add JSDocs for APIs exposed from subpath exports (#8393) by @sapphi-red
- reference: generate reference pages for APIs exposed from subpath exports (#8392) by @sapphi-red
- avoid pipe character in codeSplitting example to fix broken rendering (#8391) by @IWANABETHATGUY

### ⚡ Performance

- avoid redundant PathBuf allocations in resolve paths (#8435) by @Brooooooklyn
- bump to `sugar_path@2` (#8432) by @hyf0
- use flag-based convergence detection in include_statements (#8412) by @Brooooooklyn

### 🧪 Testing

- execute `_test.mjs` even if `executeOutput` is false (#8398) by @sapphi-red
- add retry to tree-shake/module-side-effects-proxy4 as it is flaky (#8397) by @sapphi-red
- avoid `expect.assertions()` as it is not concurrent test friendly (#8383) by @sapphi-red
- disable `mockReset` option (#8382) by @sapphi-red
- fix flaky failure caused by concurrent resolveId calls (#8381) by @sapphi-red

### ⚙️ Miscellaneous Tasks

- deps: update dependency rollup to v4.59.0 [security] (#8471) by @renovate[bot]
- ai/design: add design doc about watch mode (#8453) by @hyf0
- deps: update oxc resolver to v11.19.0 (#8461) by @renovate[bot]
- ai: introduce progressive spec-driven development pattern (#8446) by @hyf0
- deprecate output.legalComments (#8450) by @sapphi-red
- deps: update dependency oxlint-tsgolint to v0.15.0 (#8448) by @renovate[bot]
- ai: make CLAUDE.md a symlink of AGENTS.md (#8445) by @hyf0
- deps: update rollup submodule for tests to v4.59.0 (#8433) by @sapphi-red
- deps: update test262 submodule for tests (#8434) by @sapphi-red
- deps: update oxc to v0.115.0 (#8430) by @renovate[bot]
- deps: update oxc apps (#8429) by @renovate[bot]
- deps: update npm packages (#8426) by @renovate[bot]
- deps: update rust crate owo-colors to v4.3.0 (#8428) by @renovate[bot]
- deps: update github-actions (#8424) by @renovate[bot]
- deps: update rust crates (#8425) by @renovate[bot]
- deps: update oxc resolver to v11.18.0 (#8406) by @renovate[bot]
- deps: update dependency oxlint-tsgolint to v0.14.2 (#8405) by @renovate[bot]
- ban `expect.assertions` in all fixture tests (#8395) by @sapphi-red
- deps: update oxc apps (#8389) by @renovate[bot]
- ban `expect.assertions` in fixture tests (#8387) by @sapphi-red
- enable lint for `_config.ts` files (#8386) by @sapphi-red
- deps: update dependency oxlint-tsgolint to v0.14.1 (#8385) by @renovate[bot]


## [1.0.0-rc.5] - 2026-02-18

### 🚀 Features

- add `Visitor` to `rolldown/utils` (#8373) by @sapphi-red
- module-info: add `inputFormat` property to `ModuleInfo` (#8329) by @shulaoda
- default `treeshake.invalid_import_side_effects` to `false` (#8357) by @sapphi-red
- rolldown_utils: add `IndexBitSet` (#8343) by @sapphi-red
- rolldown_utils: add more methods and trait impls to BitSet (#8342) by @sapphi-red
- rolldown_plugin_vite_build_import_analysis: add support for `await import().then((m) => m.prop)` (#8328) by @sapphi-red
- rolldown_plugin_vite_reporter: support custom logger for build infos (#7652) by @shulaoda
- rust/mcs: support `entriesAwareMergeThreshold` (#8312) by @hyf0
- mcs: `maxSize` will split the oversized chunk with taking file relevance into account (#8277) by @hyf0
- rolldown_plugin_vite_import_glob: support template literal in glob import patterns (#8298) by @shulaoda
- rolldown_plugin_chunk_import_map: output importmap without spaces (#8297) by @sapphi-red
- add INEFFECTIVE_DYNAMIC_IMPORT warning in core (#8284) by @shulaoda
- mcs: generate more readable name for `entriesAware` chunks (#8275) by @hyf0
- mcs: support `entriesAware` (#8274) by @hyf0

### 🐛 Bug Fixes

- improve circular dependency detection in chunk optimizer (#8371) by @IWANABETHATGUY
- align `minify.compress: true` and `minify.mangle: true` with `minify: true` (#8367) by @sapphi-red
- rolldown_plugin_esm_external_require: apply conversion to UMD and IIFE outputs (#8359) by @sapphi-red
- cjs: bailout treeshaking on cjs modules that have multiple re-exports (#8348) by @hyf0
- handle member expression and this expression in JSX element name rewriting (#8323) by @IWANABETHATGUY
- pad `encode_hash_with_base` output to fixed length to prevent slice panics (#8320) by @shulaoda
- `xxhash_with_base` skips hashing when input is exactly 16 bytes (#8319) by @shulaoda
- complete `ImportKind::try_from` with missing variants and correct `url-import` to `url-token` (#8310) by @shulaoda
- mark Node.js builtin modules as side-effect-free when resolved via `external` config (#8304) by @IWANABETHATGUY
- mcs: `maxSize` should split chunks correctly based on sizes (#8289) by @hyf0

### 🚜 Refactor

- introduce `RawMangleOptions` and `RawCompressOptions` (#8366) by @sapphi-red
- mcs: refactor `apply_manual_code_splitting` into `ManualSplitter` (#8346) by @hyf0
- rolldown_plugin_vite_reporter: simplify hook registration and remove redundant state (#8322) by @shulaoda
- use set to store user defined entry modules (#8315) by @IWANABETHATGUY
- rust/mcs: collect groups into map at first for having clean and performant operations (#8313) by @hyf0
- mcs: introduce newtype `ModuleGroupOrigin` and `ModuleGroupId` (#8311) by @hyf0
- remove unnecessary `FinalizerMutableState` struct (#8303) by @shulaoda
- move module finalization into `finalize_modules` (#8302) by @shulaoda
- extract `apply_transfer_parts_mutation` into its own module (#8301) by @shulaoda
- move ESM format check into `determine_export_mode` (#8294) by @shulaoda
- remove `warnings` field from `GenerateContext` (#8293) by @shulaoda
- extract util function remove clippy supression (#8290) by @IWANABETHATGUY
- move `is_in_node_modules` to `PathExt` trait in `rolldown_std_utils` (#8286) by @shulaoda
- rolldown_plugin_vite_reporter: remove unnecessary ineffective dynamic import detection logic (#8285) by @shulaoda
- dev: inject hmr runtime to `\0rolldown/runtime.js` (#8234) by @hyf0
- improve naming in chunk_optimizer (#8287) by @IWANABETHATGUY
- simplify PostChunkOptimizationOperation from bitflags to enum (#8283) by @IWANABETHATGUY
- optimize BitSet.index_of_one to return iterator instead of Vec (#8282) by @IWANABETHATGUY

### 📚 Documentation

- change default value in `format` JSDoc from `'esm'` to `'es'` (#8372) by @shulaoda
- in-depth: remove `invalidImportSideEffects` option mention from lazy barrel optimization doc (#8355) by @sapphi-red
- mcs: clarify `minSize` constraints (#8279) by @ShroXd

### ⚡ Performance

- use IndexVec for chunk TLA detection (#8341) by @sapphi-red
- only invoke single resolve call for the same specifier and import kind (#8332) by @sapphi-red
- rolldown_plugin_vite_reporter: skip gzip computation when `report_compressed_size` is disabled (#8321) by @shulaoda

### 🧪 Testing

- use `vi.waitFor` and `expect.poll` instead of custom `waitUtil` function (#8369) by @sapphi-red
- rolldown_plugin_esm_external_require_plugin: add tests (#8358) by @sapphi-red
- add watch file tests (#8330) by @sapphi-red
- rolldown_plugin_vite_build_import_analysis: add test for dynamic import treeshaking (#8327) by @sapphi-red

### ⚙️ Miscellaneous Tasks

- prepare-release: skip workflow on forked repositories (#8368) by @shulaoda
- format more files (#8360) by @sapphi-red
- deps: update oxc to v0.114.0 (#8347) by @camc314
- deps: update test262 submodule for tests (#8354) by @sapphi-red
- deps: update crate-ci/typos action to v1.43.5 (#8350) by @renovate[bot]
- deps: update oxc apps (#8351) by @renovate[bot]
- rolldown_plugin_vite_reporter: remove unnecessary README.md (#8334) by @shulaoda
- deps: update npm packages (#8338) by @renovate[bot]
- deps: update rust crates (#8339) by @renovate[bot]
- deps: update dependency oxlint-tsgolint to v0.13.0 (#8337) by @renovate[bot]
- deps: update github-actions (#8336) by @renovate[bot]
- deps: update napi to v3.8.3 (#8331) by @renovate[bot]
- deps: update dependency oxlint-tsgolint to v0.12.2 (#8325) by @renovate[bot]
- remove unnecessary transform.decorator (#8314) by @IWANABETHATGUY
- deps: update dependency rust to v1.93.1 (#8305) by @renovate[bot]
- deps: update dependency oxlint-tsgolint to v0.12.1 (#8300) by @renovate[bot]
- deps: update oxc apps (#8296) by @renovate[bot]
- docs: don't skip for build runs without cache (#8281) by @sapphi-red


## [1.0.0-rc.4] - 2026-02-11

### 🚀 Features

- rename error name to `RolldownError` from `RollupError` (#8262) by @sapphi-red
- add hidden `resolve_tsconfig` function for Vite (#8257) by @sapphi-red
- rust: introduce `rolldown_watcher` (#8161) by @hyf0
- unify `comments` and `legalComments` into a single granular `comments` option (#8229) by @IWANABETHATGUY
- add builtin plugin for visualizing chunk graph (#8162) by @IWANABETHATGUY
- show import declaration location in AssignToImport errors (#8222) by @Copilot
- show import declaration span in CannotCallNamespace error (#8223) by @Copilot
- emit error when plugin accidentally removes runtime module symbols (#8203) by @IWANABETHATGUY
- support tsconfig loading & inputMap for `transform` (#8180) by @sapphi-red
- rolldown_plugin_vite_reporter: update warning message to link to Rolldown docs (#8205) by @sapphi-red

### 🐛 Bug Fixes

- avoid panic on untranspiled JSX syntax by reporting a diagnostic error (#8226) by @IWANABETHATGUY
- rolldown_plugin_vite_import_glob: relax absolute path check and improve invalid glob warning (#8219) by @shulaoda
- merge chunks after detect circular reference (#8154) by @IWANABETHATGUY
- rust: detect runtime module side effects based on its content (#8209) by @hyf0

### 🚜 Refactor

- rename `other` to `jsdoc` in comments options (#8256) by @IWANABETHATGUY
- rename chunk-visualize plugin with bundle-analyzer plugin (#8255) by @IWANABETHATGUY
- remove EXPORT_UNDEFINED_VARIABLE error (#8228) by @Copilot
- consolidate missing runtime symbol errors into a single diagnostic (#8220) by @IWANABETHATGUY
- stabilize `parse` and `parseSync` (#8215) by @sapphi-red
- return errors instead of panicking on builtin plugin conversion failure (#8217) by @shulaoda
- expose `parse` / `minify` / `transform` from `rolldown/utils` (#8214) by @sapphi-red
- prepare defer chunk merging (#8153) by @IWANABETHATGUY

### 📚 Documentation

- remove `<script>` escape behavior difference note from `platform` option (#8253) by @sapphi-red
- TypeScript & JSX support by plugins (#8183) by @sapphi-red

### 🧪 Testing

- ensure runtime module is preserved even if it's not used but has side effects (#8213) by @hyf0

### ⚙️ Miscellaneous Tasks

- deps: update oxc to v0.113.0 (#8267) by @renovate[bot]
- deps: update dependency oxlint-tsgolint to v0.12.0 (#8272) by @renovate[bot]
- deps: update oxc apps (#8269) by @renovate[bot]
- deps: update test262 submodule for tests (#8261) by @sapphi-red
- deps: update crate-ci/typos action to v1.43.4 (#8260) by @renovate[bot]
- deps: update dependency esbuild to v0.27.3 (#8250) by @renovate[bot]
- deps: update rust crates (#8244) by @renovate[bot]
- deps: update dependency semver to v7.7.4 (#8247) by @renovate[bot]
- deps: update github-actions (#8243) by @renovate[bot]
- deps: update npm packages (#8245) by @renovate[bot]
- deps: update oxc resolver to v11.17.1 (#8240) by @renovate[bot]
- deps: update rust crate oxc_sourcemap to v6.0.2 (#8241) by @renovate[bot]
- rust: handle ignored `RUSTSEC-2025-0141` cargo check error (#8235) by @hyf0
- deps: update dependency oxlint-tsgolint to v0.11.5 (#8233) by @renovate[bot]
- deps: update dependency rolldown-plugin-dts to ^0.22.0 (#8232) by @renovate[bot]
- deps: update crate-ci/typos action to v1.43.3 (#8225) by @renovate[bot]
- deps: update dependency rolldown-plugin-dts to v0.21.9 (#8224) by @renovate[bot]
- deps: update crate-ci/typos action to v1.43.2 (#8212) by @renovate[bot]
- remove rolldown_plugin_vite_wasm_helper (#8207) by @shulaoda
- build docs for production (#8206) by @sapphi-red


## [1.0.0-rc.3] - 2026-02-04

### 🚀 Features

- expose `RUNTIME_MODULE_ID` constant for plugin authors (#8199) by @shulaoda
- warn unsupported combination of `preserveValueImports` and `importsNotUsedAsValues` in tsconfig.json (#8169) by @sapphi-red
- sophisticated watch tracking for load dependencies (#8092) by @sapphi-red
- add `inputMap` option to `minify` / `minifySync` functions (#8138) by @sapphi-red
- consolidate same tsconfig errors (#8119) by @sapphi-red
- include tsconfig file path in error messages (#8107) by @Copilot
- lazy-barrel: support incremental build mode (#8114) by @shulaoda

### 🐛 Bug Fixes

- rust: preserve dependencies added by `this.addWatchFile` (#8198) by @hyf0
- spawn `RuntimeModuleTask` after `build_start` to avoid race condition (#8182) by @shulaoda
- rust/dev: only close after the ongoing task finished (#8147) by @hyf0
- ensure `\0rolldown/runtime.js` will go through transform hook and add test (#8093) by @hyf0
- `[name]` in `assetFileNames` does not include the directory part (#8098) by @IWANABETHATGUY
- handle external module properties in namespace imports (#8124) by @IWANABETHATGUY
- keep user-defined entry modules in their own chunks (#8047) by @IWANABETHATGUY
- avoid `Unknown resolve error` error message (#8111) by @sapphi-red

### 💼 Other

- remove warnings  for building rolldown with `not(feature = "experimental")` (#8110) by @coolreader18

### 🚜 Refactor

- move `VERSION` to `constants` directory (#8200) by @shulaoda
- simplify import symbol check using `SymbolFlags` (#8193) by @shulaoda
- extract tsconfig option and transform options merging logic (#8168) by @sapphi-red
- filter empty module_groups before sorting (#8149) by @ShroXd
- lazy-barrel: use single `remove` instead of `contains_key` + `remove` (#8123) by @shulaoda
- lazy-barrel: avoid redundant call and inline `get_barrel_normal_module` (#8122) by @shulaoda
- use logger instead of console.log for warnings (#8117) by @IWANABETHATGUY
- module-loader: remove intermediate ModuleTaskOwnerRef type (#8113) by @shulaoda
- rename ReExportExternalModule to ReExportDynamicExports (#8104) by @IWANABETHATGUY

### 📚 Documentation

- add dynamic OG image generation (#8192) by @sapphi-red
- add dynamic OG image generation (#8191) by @sapphi-red
- add dynamic OG image generation (#8179) by @Copilot
- apis: add links to option descriptions in JSDoc comments (#8167) by @sapphi-red
- apis: clarify parameters of `resolveDynamicImport` hook (#8137) by @sapphi-red
- lazy-barrel: clarify default export behavior (#8128) by @shulaoda

### ⚡ Performance

- remove unnecessary assignment for default export (#8127) by @shulaoda

### 🧪 Testing

- dev: `this.addWatchFile` dependency should be preserved after reload (#8165) by @sapphi-red
- mark flaky `transform_runtime_module` test as ignored (#8178) by @Copilot
- rolldown_sourcemap: add test for coarse segments (#8166) by @sapphi-red
- dev: correctly assert file change (#8164) by @sapphi-red
- rust: `transform_runtime_module` test shouldn't panic inside (#8151) by @hyf0
- rust: fix flakiness of rust tests (#8150) by @hyf0
- mark `output.dynamicImportInCjs` tests as passed (#8125) by @shulaoda
- lazy-barrel: add test cases for default export (#8129) by @shulaoda
- rolldown_plugin_vite_manifest: use relative path for outPath (#8101) by @shulaoda

### ⚙️ Miscellaneous Tasks

- deps: update crate-ci/typos action to v1.43.1 (#8188) by @renovate[bot]
- deps: update rust crate ts-rs to v12 (#8160) by @renovate[bot]
- deps: update crate-ci/typos action to v1.43.0 (#8175) by @renovate[bot]
- deps: update rust crates (#8157) by @renovate[bot]
- deps: update oxc to v0.112.0 (#8171) by @renovate[bot]
- deps: update rollup submodule for tests to v4.57.1 (#8176) by @sapphi-red
- deps: update test262 submodule for tests (#8177) by @sapphi-red
- deps: update dependency oxlint to v1.43.0 (#8173) by @renovate[bot]
- deps: update dependency oxfmt to ^0.28.0 (#8170) by @renovate[bot]
- deps: update dependency rolldown-plugin-dts to v0.21.8 (#8163) by @renovate[bot]
- deps: update dependency vue-router to v5 (#8159) by @renovate[bot]
- deps: update github-actions (#8158) by @renovate[bot]
- deps: update npm packages (#8156) by @renovate[bot]
- deps: update dependency oxlint-tsgolint to v0.11.4 (#8140) by @renovate[bot]
- fix clippy replacement typo (#8136) by @IWANABETHATGUY
- disallow `HashMap::new` and `HashSet::new` by clippy (#8135) by @sapphi-red
- deps: update dependency rolldown-plugin-dts to v0.21.7 (#8126) by @renovate[bot]
- deps: update oxc resolver to v11.17.0 (#8121) by @renovate[bot]
- deps: update dependency oxlint-tsgolint to v0.11.3 (#8109) by @renovate[bot]

### ❤️ New Contributors

* @coolreader18 made their first contribution in [#8110](https://github.com/rolldown/rolldown/pull/8110)


## [1.0.0-rc.2] - 2026-01-28

### 💥 BREAKING CHANGES

- expose `\0rolldown/runtime` in transform hook (#8068) by @hyf0
- rename `rolldown:runtime` to `\0rolldown/runtime.js` (#8067) by @hyf0

### 🚀 Features

- remove inlined constants in smart mode (#8085) by @sapphi-red
- allow more options for `this.emitFile` with `type: 'prebuilt-chunk'` (#8062) by @sapphi-red
- warn when both code and postBanner contain shebang (#8039) by @Copilot

### 🐛 Bug Fixes

- update the links to Rolldown docs in the error messages (#8103) by @sapphi-red
- handle tsconfig.json load errors (#8105) by @sapphi-red
- include inlined constants in namespace object (#8099) by @sapphi-red
- vite test ci (#8084) by @IWANABETHATGUY
- renamer: nested binding shadowing external module namespace in UMD/IIFE formats (#8083) by @Dunqing
- deduplicate ESM chunk imports by canonical symbol (#8059) by @IWANABETHATGUY
- refine side-effect detection for BigInt and RegExp (#8060) by @IWANABETHATGUY
- rust: use string literal span for `new URL` error diagnostic (#8043) by @valadaptive
- rust: use ModuleType::Asset for `new URL` imports (#8035) by @valadaptive
- CJS-ESM interop - property assignment on CJS module exports (#8006) by @IWANABETHATGUY
- eliminate the facade chunk if the dynamic entry module has been merged into common chunk (#8046) by @IWANABETHATGUY
- Inlining dynamic imports broken with multiple entry points (#8037) by @IWANABETHATGUY
- devtools: revert `Chunk#id` to `Chunk#chunk_id` (#8040) by @hyf0
- invert `__exportAll` parameter logic to reduce default output size (#8036) by @Copilot
- `</script` tag search should be case insensitive (#8033) by @IWANABETHATGUY
- use directory name as-is for the variable name even if the name contained `.` (#8029) by @Copilot
- dev/lazy: remove unnecessary rewrite from top level `this` to `undefined` (#8020) by @hyf0
- dev/lazy: should keep lazy entries imports for patch file (#8019) by @hyf0
- `output.generatedCode.preset: 'es2015'` was not set by default (#8026) by @sapphi-red
- node: align option validator to types (#8023) by @sapphi-red
- node: allow `output.strictExecutionOrder` by the option validator (#8022) by @sapphi-red
- types: return `this` from on / off methods of `RolldownWatcher` (#8015) by @sapphi-red

### 🚜 Refactor

- rolldown_plugin_vite_dynamic_import_vars: remove v1 implementation (#8096) by @shulaoda
- rolldown_plugin_vite_import_glob: remove v1 implementation (#8095) by @shulaoda
- lazy-barrel: restructure lazy barrel implementation (#8070) by @shulaoda
- remove `use_built_ins` and `use_spread` from internal JSX options (#8079) by @sapphi-red
- remove `experimental.transformHiresSourcemap` (#8055) by @Copilot
- rust: use `is_data_url` more consistently (#8042) by @valadaptive
- use `FxIndexMap` to store EntryPoint (#8032) by @IWANABETHATGUY
- node: add type checks that ensures validator schema is up to date with types (#8024) by @sapphi-red

### 📚 Documentation

- link to vite plugin registry (#8086) by @sapphi-red
- lazy-barrel: improve documentation and enable in sidebar (#8072) by @shulaoda
- add more examples and details (#8054) by @sapphi-red
- in-depth: add dead code elimination page (#8007) by @sapphi-red
- update status from beta to release candidate (#8012) by @shulaoda

### ⚡ Performance

- run inline-const pass for modules that are affected by inlining (#8064) by @sapphi-red

### 🧪 Testing

- lazy-barrel: use package.json sideEffects instead of plugin hook (#8077) by @shulaoda
- lazy-barrel: enable tests and add treeshake-behavior cases (#8071) by @shulaoda

### ⚙️ Miscellaneous Tasks

- deps: update crate-ci/typos action to v1.42.3 (#8087) by @renovate[bot]
- deps: update rollup submodule for tests to v4.56.0 (#8073) by @sapphi-red
- deps: update oxc to v0.111.0 (#8063) by @renovate[bot]
- deps: update dependency rolldown-plugin-dts to v0.21.6 (#8076) by @renovate[bot]
- deps: update test262 submodule for tests (#8074) by @sapphi-red
- deps: update crate-ci/typos action to v1.42.2 (#8069) by @renovate[bot]
- deps: update oxc apps (#8066) by @renovate[bot]
- remove `{@include ./foo.md}` from d.ts files (#8056) by @sapphi-red
- deps: update dependency oxlint-tsgolint to v0.11.2 (#8057) by @renovate[bot]
- deps: update github-actions (#8050) by @renovate[bot]
- deps: update npm packages (#8051) by @renovate[bot]
- deps: update rust crates (#8049) by @renovate[bot]
- debug: add IdxExt debug trait for human-readable index debugging (#8045) by @IWANABETHATGUY
- deps: update dependency rolldown-plugin-dts to v0.21.5 (#8034) by @renovate[bot]
- deps: update oxc resolver to v11.16.4 (#8031) by @renovate[bot]
- deps: update dependency rolldown-plugin-dts to v0.21.4 (#8030) by @renovate[bot]
- deps: update dependency rust to v1.93.0 (#8018) by @renovate[bot]
- archive 2025 beta changelog (#8014) by @shulaoda
- update release workflow version pattern from beta to rc (#8013) by @shulaoda

### ❤️ New Contributors

* @valadaptive made their first contribution in [#8043](https://github.com/rolldown/rolldown/pull/8043)


## [1.0.0-rc.1] - 2026-01-22

### 🚀 Features

- debug_info: add facade chunk elimination reason (#7980) by @IWANABETHATGUY
- support lazy barrel optimization (#7933) by @shulaoda
- add `experimental.lazyBarrel` option (#7908) by @shulaoda
- skip unused external modules from IIFE parameter list (#7978) by @sapphi-red
- add custom panic hook for better crash reporting (#7752) by @shulaoda
- treeshake: add `invalidImportSideEffects` option (#7958) by @shulaoda
- merge allow-extension emitted chunks (#7940) by @IWANABETHATGUY
- nativeMagicString generateMap (#7944) by @IWANABETHATGUY
- Include meta.magicString in RenderChunkMeta (#7943) by @IWANABETHATGUY
- debug_info: add debug info for eliminated facade chunks (#7946) by @IWANABETHATGUY
- stablize `strictExecutionOrder` and move to `output.strictExecutionOrder` from `experimental.strictExecutionOrder` (#7901) by @sapphi-red
- add documentation link to require() error message (#7898) by @Copilot
- add `codeSplitting: boolean` and deprecate `inlineDynamicImports` (#7870) by @hyf0
- dev: change lazy module URL to `/@vite/lazy` from `/lazy` (#7884) by @sapphi-red

### 🐛 Bug Fixes

- transform JS files containing `</script>` to escape template literals (#7987) by @IWANABETHATGUY
- apply avoid-breaking-exported-api = false to clippy.toml and fix clippy errors (#7982) by @Boshen
- pass `kind` from `this.resolve` (#7981) by @sapphi-red
- rolldown_plugin_vite_resolve: ignore yarn resolution errors and fallback to other resolvers (#7968) by @sapphi-red
- renamer: prevent renaming symbols when there no conflicts (#7936) by @Dunqing
- correct minifyInterExports when emitted chunk got merged (#7941) by @IWANABETHATGUY
- deduplicate entry points when module is both emitted and dynamically imported (#7885) by @IWANABETHATGUY
- dev: add `@vite-ignore` to lazy compilation proxy module import (#7883) by @sapphi-red

### 🚜 Refactor

- rust: enable clippy nursery lint group (#8002) by @Boshen
- rust: fix inconsistent_struct_constructor clippy lint (#7999) by @Boshen
- rust: fix needless_pass_by_ref_mut clippy lint (#7994) by @Boshen
- rust: fix unnecessary_wraps clippy lint (#7993) by @Boshen
- rust: fix enum_variant_names clippy lint (#7992) by @Boshen
- fix single_match clippy lint (#7997) by @Boshen
- rust: fix redundant_clone clippy lint (#7996) by @Boshen
- rust: rename CJS to Cjs to follow upper_case_acronyms lint (#7991) by @Boshen
- rust: remove unnecessary Box wrapper around Vec in MemberExprRef (#7990) by @Boshen
- import_record: make resolved_module optional (#7907) by @shulaoda
- remove unnecessary `.parse` (#7966) by @sapphi-red
- remove unused `ImportRecordMeta::IsPlainImport` (#7948) by @shulaoda
- proper set chunk meta (#7939) by @IWANABETHATGUY
- module_loader: remove `try_spawn_with_cache` (#7920) by @shulaoda
- link_stage: simplify `ImportStatus::NoMatch` to unit variant (#7909) by @shulaoda
- improve global scope symbol reservation in chunk deconfliction (#7906) by @IWANABETHATGUY
- simplify ast unwrapping in generate stage (#7900) by @IWANABETHATGUY
- generate_stage: optimize cross-chunk imports computation (#7889) by @shulaoda
- link_stage: move runtime require logic into match branch (#7892) by @shulaoda
- link_stage: simplify runtime require reference conditions (#7891) by @shulaoda
- link_stage: inline and simplify external dynamic import check (#7890) by @shulaoda
- generate_stage: simplify external module import collection logic (#7887) by @shulaoda
- avoid redundant module lookup in TLA computation (#7886) by @shulaoda
- dev: `devEngine.compileEntry` does not return null (#7882) by @sapphi-red
- dev: fix type errors for test HMR runtime (#7881) by @sapphi-red
- dev: move `clientId` property to `DevRuntime` base class (#7880) by @sapphi-red
- dev: generate client id in browser (#7878) by @hyf0

### 📚 Documentation

- apis: organize hook filters documentation and add composable filters section (#8003) by @sapphi-red
- update `vitepress-plugin-group-icons` (#7947) by @yuyinws
- add in-depth documentation for lazy barrel optimization (#7969) by @shulaoda
- bump theme & update activeMatch for reference (#7963) by @mdong1909
- mark `build()` API as experimental (#7954) by @sapphi-red
- enhance search functionality with improved scoring and filtering logic (#7935) by @hyf0
- add minor comments to multiple types (#7930) by @sapphi-red
- refactor advanedChunks related content to adapt manual code splitting concept (#7925) by @hyf0
- apis: add content to Bundler API page (#7926) by @sapphi-red
- apis: restructure plugin API related docs (#7924) by @sapphi-red
- add plugin API docs (#7923) by @sapphi-red
- apis: add docs to important APIs (#7913) by @sapphi-red
- move the important APIs to the top of the sidebar (#7912) by @sapphi-red
- apis: add more content to CLI documentation (#7911) by @sapphi-red
- apis: generate CLI docs from --help output (#7910) by @sapphi-red
- add fathom analytics (#7896) by @mdong1909

### ⚡ Performance

- use u32 for string indices in string_wizard and rolldown to reduce memory usage (#7989) by @IWANABETHATGUY
- rust: remove all usages of `with_scope_tree_child_ids(true)` for `SemanticBuilder` (#7995) by @Dunqing
- renamer: skip unnecessary nested scope symbol processing (#7899) by @Dunqing
- module_loader: use ArcStr for importer_id to avoid string copy (#7922) by @shulaoda
- module_loader: defer `ModuleTaskOwner` construction until needed (#7921) by @shulaoda
- renamer: optimize symbol renaming by eliminating `rename_non_root_symbol` pass (#7867) by @Dunqing

### 🧪 Testing

- add lazy barrel optimization test cases (#7967) by @shulaoda

### ⚙️ Miscellaneous Tasks

- remove lazy barrel option (#8010) by @shulaoda
- mark watch API as experimental (#8004) by @sapphi-red
- deps: update dependency lodash-es to v4.17.23 [security] (#8001) by @renovate[bot]
- git ignore zed local config (#7988) by @IWANABETHATGUY
- setup publint for published packages (#7972) by @Copilot
- enable `tagged_template_transform ` uncondionally (#7975) by @IWANABETHATGUY
- deps: update oxc to v0.110.0 (#7964) by @renovate[bot]
- deps: update oxc apps (#7962) by @renovate[bot]
- ai: add upgrade-oxc Claude skill (#7957) by @Boshen
- deps: update rollup submodule for tests to v4.55.2 (#7959) by @sapphi-red
- deps: update test262 submodule for tests (#7960) by @sapphi-red
- deps: update crate-ci/typos action to v1.42.1 (#7961) by @renovate[bot]
- deps: update rust crates (#7951) by @renovate[bot]
- deps: update npm packages (#7953) by @renovate[bot]
- deps: update github-actions (#7952) by @renovate[bot]
- deps: update npm packages (#7950) by @renovate[bot]
- format magic-string test before write to disk (#7945) by @IWANABETHATGUY
- deps: update dependency rolldown-plugin-dts to ^0.21.0 (#7915) by @renovate[bot]
- deps: update dependency oxlint-tsgolint to v0.11.1 (#7914) by @renovate[bot]
- deps: update dependency diff to v8.0.3 [security] (#7904) by @renovate[bot]
- remove outdated TODO comment in `collect_depended_symbols` (#7888) by @shulaoda
- deps: update oxc resolver to v11.16.3 (#7876) by @renovate[bot]


## [1.0.0-beta.60] - 2026-01-14

### 💥 BREAKING CHANGES

- tsconfig: enable auto-discovery by default (#7817) by @shulaoda

### 🚀 Features

- distinguish transformer diagnostics from parse errors (#7872) by @shulaoda
- emit transformer warnings instead of ignoring them (#7850) by @shulaoda
- node: add `output.codeSplitting` option and deprecate `output.advancedChunks` (#7855) by @hyf0
- nativeMagicString reset (#7828) by @IWANABETHATGUY
- nativeMagicString lastChar (#7819) by @IWANABETHATGUY
- dev/lazy: inject lazy compilation runtime automatically (#7816) by @hyf0
- nativeMagicString snip (#7818) by @IWANABETHATGUY
- nativeMagicString construct with options (#7814) by @IWANABETHATGUY
- nativeMagicString clone (#7813) by @IWANABETHATGUY
- nativeMagicString `insert` (#7812) by @IWANABETHATGUY
- nativeMagicString `slice` (#7807) by @IWANABETHATGUY
- nativeMagicString trim methods (#7800) by @IWANABETHATGUY
- make closeBundle hook receive the last error (#7278) by @Copilot

### 🐛 Bug Fixes

- when package only contains export default, cjsDefault didn't resolve correctly (#7873) by @IWANABETHATGUY
- inline __name calls for default exports  (#7862) by @IWANABETHATGUY
- improve variable renaming to avoid unnecessary shadowing in nested scopes (#7859) by @IWANABETHATGUY
- use correct index when inserting keepNames statements during export default transformation (#7853) by @IWANABETHATGUY
- transform non-static dynamic imports when `dynamicImportInCjs` is `false` (#7823) by @shulaoda
- dev/lazy: should include imported and non-executed modules in the patch (#7815) by @hyf0
- set ExportsKind to Esm when json is none object literal  (#7808) by @IWANABETHATGUY
- nativeMagicString move api (#7796) by @IWANABETHATGUY
- remove unnecessary exports after merging into commong and user defined entry (#7789) by @IWANABETHATGUY
- use output.name instead of chunk.name in mixed export warning (#7788) by @Copilot

### 🚜 Refactor

- generalize ParseError to OxcError with dynamic EventKind (#7868) by @shulaoda
- rust: rename `advanced_chunks` to `manual_code_splitting` (#7856) by @hyf0
- string_wizard error hanlding (#7830) by @IWANABETHATGUY
- remove `experimental.disableLiveBindings` option (#7820) by @sapphi-red
- node/test: run fixture tests in concurrent (#7790) by @hyf0
- move ConfigExport and RolldownOptionsFunction types to define-config (#7799) by @shulaoda
- cli: validate config after resolving and improve error message (#7798) by @shulaoda

### 📚 Documentation

- rebrand (#7670) by @yyx990803
- fix incorrect default value for propertyReadSideEffects (#7847) by @Copilot
- remove options pages and redirect to reference pages (#7834) by @sapphi-red
- options: inline types to option property pages (#7831) by @sapphi-red
- options: port checks.pluginTimings content from options page to reference page (#7832) by @sapphi-red
- options: use `@linkcode` where possible (#7824) by @sapphi-red
- options: port content from options page to reference page (#7822) by @sapphi-red
- options: add descriptions for output options (#7821) by @sapphi-red
- options: add description for input options (#7802) by @sapphi-red
- options: add description for `checks.*` (#7801) by @sapphi-red
- apis: add hook graph (#7671) by @sapphi-red

### 🧪 Testing

- add all valid combination of chunk exports related test (#7851) by @IWANABETHATGUY
- enable MagicString test after api return type alignment (#7797) by @IWANABETHATGUY
- init magic-string test (#7794) by @IWANABETHATGUY

### ⚙️ Miscellaneous Tasks

- vite-tests: configure git user for rebase operation (#7875) by @shulaoda
- rolldown_binding: remove v3 native plugins (#7837) by @shulaoda
- rolldown_binding: allow crate-type as lib (#7866) by @Brooooooklyn
- README.md: adjust position and size of rolldown logo (#7861) by @hyf0
- deps: update test262 submodule for tests (#7857) by @sapphi-red
- deps: update oxc to v0.108.0 (#7845) by @renovate[bot]
- deps: update dependency oxlint to v1.39.0 (#7849) by @renovate[bot]
- deps: update dependency oxfmt to ^0.24.0 (#7844) by @renovate[bot]
- deps: update npm packages (#7841) by @renovate[bot]
- deps: update rust crates (#7839) by @renovate[bot]
- deps: update github-actions (#7840) by @renovate[bot]
- use workspace edition for all crates (#7829) by @IWANABETHATGUY
- deps: update dependency oxlint-tsgolint to v0.11.0 (#7827) by @renovate[bot]
- deps: update napi to v3.8.2 (#7810) by @renovate[bot]
- remove outdated snapshot files (#7806) by @shulaoda
- deps: update crate-ci/typos action to v1.42.0 (#7792) by @renovate[bot]


## [1.0.0-beta.59] - 2026-01-07

### 🚀 Features

- plugin_timings: add 3s threshold and doc link to warning message (#7741) by @shulaoda
- improve treeshaking logic to handle empty parameter list in dynamic import .then() callbacks (#7781) by @Copilot
- dev/lazy: don't include already executed modules (#7745) by @hyf0
- dev/lazy: support dynamic `import(..)` (#7726) by @hyf0
- inline dynamic imports that imports statically imported modules (#7742) by @IWANABETHATGUY
- option: add experimental option to control chunk optimization (#7738) by @IWANABETHATGUY

### 🐛 Bug Fixes

- inline dynamic entry to user defined entry with esm wrap kind (#7783) by @IWANABETHATGUY
- use canonical namespace reference for property access (#7777) by @IWANABETHATGUY
- dynamic entry merged into common chunk with cjs and esm wrap kind (#7771) by @IWANABETHATGUY
- tla: should not await non-tla-related modules (#7768) by @hyf0
- dynamic entry captured by common chunk with CJS format (#7757) by @IWANABETHATGUY
- module_loader: mark emitted chunks as user-defined entry when already loaded (#7765) by @shulaoda
- normalize preserveModulesRoot path (#7737) by @IWANABETHATGUY
- linker: resolve race condition in side effects computation for export-star (#7728) by @camc314

### 🚜 Refactor

- plugin_timings: filter out plugins with duration < 1s from timing warnings (#7785) by @shulaoda
- module_loader: remove unnecessary collect before extend (#7769) by @shulaoda
- rename _id suffixes to _idx for oxc_index types (#7767) by @IWANABETHATGUY
- remove duplicate `preserve_entry_signatures` from `AddEntryModuleMsg` (#7762) by @shulaoda
- module_loader: pass `user_defined_entries` by reference (#7756) by @shulaoda
- dev/lazy: get proxy entry's `ResolvedId` correctly (#7746) by @hyf0
- simplify try_rewrite_import_expression control flow (#7753) by @IWANABETHATGUY
- module_loader: remove unnecessary dynamic import handling for runtime module (#7754) by @shulaoda
- inline __toDynamicImportESM  (#7747) by @IWANABETHATGUY
- use From impl for ModuleLoaderOutput conversion (#7732) by @shulaoda
- remove duplicate fields from `ModuleLoader` (#7731) by @shulaoda
- tweak `resolve_user_defined_entries` (#7727) by @shulaoda

### 📚 Documentation

- add rolldown-string reference to native MagicString compatibility section (#7778) by @Copilot
- improve comments for export star side effects handling (#7730) by @IWANABETHATGUY

### 🧪 Testing

- use assertion instead of console.log for some testcase (#7744) by @IWANABETHATGUY

### ⚙️ Miscellaneous Tasks

- tweak some `output.dynamicImportInCjs` related rollup test results (#7776) by @sapphi-red
- mark esbuild/dce/dce_of_symbol_ctor_call as passed (#7775) by @sapphi-red
- deps: update oxc apps (#7772) by @renovate[bot]
- vite-tests: allow running on PRs with `test: vite-tests` label (#7770) by @shulaoda
- deps: update oxc apps (#7760) by @renovate[bot]
- deps: update rollup submodule for tests to v4.55.1 (#7763) by @sapphi-red
- deps: update test262 submodule for tests (#7764) by @sapphi-red
- deps: update oxc to v0.107.0 (#7758) by @camc314
- deps: update taiki-e/install-action action to v2.65.13 (#7751) by @renovate[bot]
- deps: update rust crates (#7750) by @renovate[bot]
- deps: update npm packages (#7749) by @renovate[bot]
- deps: update github-actions (#7748) by @renovate[bot]
- deps: update dependency oxlint-tsgolint to v0.10.1 (#7729) by @renovate[bot]
- deps: update crate-ci/typos action to v1.41.0 (#7725) by @renovate[bot]
