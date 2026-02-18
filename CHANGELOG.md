
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
