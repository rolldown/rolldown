
## [1.0.0-beta.23] - 2025-07-01

### 💥 BREAKING CHANGES

- debug: rename debug logs fields to be consitent (#5125) by @antfu

### 🚀 Features

- debug: append `content` for asset data and unify `source` field to `content`. (#5130) by @hyf0
- debug: emit asset-related data (#5124) by @hyf0
- commonjs tree shaking. (#5095) by @IWANABETHATGUY
- rolldown_plugin_wasm_helper: align behaviors for expected functionality (#5120) by @shulaoda
- remove `experimental.enableComposingJsPlugins` (#5122) by @shulaoda
- use same option object reference (#5114) by @sapphi-red
- add util function to inspect why the facade symbol is created (#5115) by @IWANABETHATGUY
- types: expose `ChunkingContext` type (#5112) by @sapphi-red
- scan commonjs exports (#5111) by @IWANABETHATGUY
- debug: remove unneeded source info of render chunk events (#5109) by @hyf0
- debug: only emit debug information for trace level (#5108) by @hyf0
- debug: emit chunk related informations (#5106) by @hyf0

### 🐛 Bug Fixes

- debug: add hook render chunk start and end events to Meta enum (#5117) by @hyf0

### 🚜 Refactor

- rust: unify outdated namings (#5127) by @hyf0

### 📚 Documentation

- advanced-chunks: clarify module capturing behavior (#5129) by @hyf0

### 🧪 Testing

- support array output options (#5113) by @sapphi-red

### ⚙️ Miscellaneous Tasks

- remove unused code (#5128) by @IWANABETHATGUY
- remove unnecessary `skipComposingJsPlugin` (#5123) by @shulaoda
- deps: update crate-ci/typos action to v1.34.0 (#5119) by @renovate[bot]
- deps: update NAPI-RS to 3.0.0-beta.11 (#5110) by @shulaoda


## [1.0.0-beta.22] - 2025-06-30

### 🚀 Features

- plugin: use `Log` instead of `BuildDiagnostic` for `PluginContext#log` (#5099) by @shulaoda
- plugin: support info/warn/debug in native plugin context (#5098) by @shulaoda
- plugin: support vite plugin custom in `PluginContext#resolve` (#5097) by @shulaoda
- rolldown_plugin_import_glob: align edge cases with `rolldown-vite` (#5094) by @shulaoda
- rolldown_plugin_vite_resolve: align subpath import resolution (#5093) by @shulaoda
- add commonjs tree shake option (#5096) by @IWANABETHATGUY
- rolldown_plugin_web_worker_post: align with `rolldown-vite` (#5092) by @shulaoda
- manual-chunks: support `getModuleInfo` (#5087) by @hyf0
- advanced-chunks: support `getModuleInfo` in `groups.{group}.name` (#5086) by @hyf0

### 🚜 Refactor

- rolldown_loader_utils: inline unnecessary `binary_to_esm` (#5100) by @shulaoda

### ⚡ Performance

- hmr: remove `console.debug` in `runtime.registerModule` (#5091) by @sapphi-red
- hmr: remove no-op `runtime.__toCommonJS` call (#5089) by @sapphi-red

### ⚙️ Miscellaneous Tasks

- deps: lock file maintenance rust crates (#5105) by @renovate[bot]
- deps: lock file maintenance npm packages (#5104) by @renovate[bot]
- deps: update rust crate schemars to v1 (#5103) by @renovate[bot]
- deps: update dependency vite to v7 (#5102) by @renovate[bot]
- deps: update github-actions (#5101) by @renovate[bot]
- deps: update dependency rolldown-plugin-dts to v0.13.12 (#5088) by @renovate[bot]


## [1.0.0-beta.21] - 2025-06-27

### 🚀 Features

- rolldown: oxc_resolver v11.3.0 (#5078) by @Boshen
- debug: support to emit `meta.json` (#5075) by @hyf0
- plugin/vite-resolve: support tsconfig paths (#5004) by @sapphi-red

### 🐛 Bug Fixes

- plugin/vite-resolve: resolve glob side effects field correctly (#5079) by @sapphi-red
- rolldown_plugin_transform: don't load tsconfig for files in node_modules (#5074) by @sapphi-red
- preserveModules: correctly generate absolute chunk file names (#5072) by @shulaoda
- vite-tests: fix `pnpm install` failure (#5070) by @shulaoda

### 📚 Documentation

- contrib-guide: add memory profile steps (#5053) by @sapphi-red

### ⚙️ Miscellaneous Tasks

- remove `just lint` in `packages/debug/package.json` (#5080) by @hyf0
- deps: update dependency rolldown-plugin-dts to v0.13.12 (#5077) by @renovate[bot]
- repo: make `pnpm lint-knip` part of `just lint-node` (#5076) by @hyf0
- infra: move `insta.yaml` to the workspace root (#5073) by @shulaoda


## [1.0.0-beta.20] - 2025-06-25

### 🚀 Features

- rolldown: oxc v0.75.0 (#5069) by @Boshen
- rolldown: oxc 0.74.0 (#5047) by @Boshen

### 🐛 Bug Fixes

- vite-tests: avoid crash when `packageJson.pnpm` is undefined (#5066) by @shulaoda
- JSX preset options not working correctly (#5060) by @shulaoda
- remove improper jsdoc of  builtin define plugin (#5046) by @AliceLanniste

### 🚜 Refactor

- passing vec length directly (#5050) by @ShroXd

### 📚 Documentation

- explain that a chunk may be bigger than `maxSize` (#5014) by @sapphi-red
- update example about `withFilter` (#5040) by @IWANABETHATGUY

### ⚡ Performance

- preallocate smaller vec for hash placeholder finder (#5064) by @sapphi-red
- reduce temporary memory usage by avoiding output chunk clone until needed (#5058) by @sapphi-red
- skip empty write bundle hook with `hook_usage` meta (#5057) by @sapphi-red

### ⚙️ Miscellaneous Tasks

- deps: update dependency rolldown-plugin-dts to v0.13.12 (#5065) by @renovate[bot]
- rolldown: upgrade NAPI-RS to 3.0.0-beta.10 (#5063) by @Brooooooklyn
- bump oxc-resolver to v11.2.1 (#5061) by @Boshen
- deps: update dependency tsdown to v0.12.9 (#5056) by @renovate[bot]
- deps: update dependency rolldown-plugin-dts to v0.13.12 (#5054) by @renovate[bot]
- remove long-commented-out code (#5048) by @shulaoda
- deps: lock file maintenance npm packages (#5042) by @renovate[bot]
- deps: lock file maintenance rust crates (#5043) by @renovate[bot]
- deps: update taiki-e/install-action action to v2.54.0 (#5045) by @renovate[bot]
- deps: update taiki-e/install-action action to v2.53.2 (#5041) by @renovate[bot]


## [1.0.0-beta.19] - 2025-06-22

### 🚀 Features

- support `OutputOptions#manualChunks` (#5037) by @hyf0
- advanced-chunks: support `advancedChunks#gruop#name` to be function (#5035) by @hyf0
- rolldown_plugin_import_glob: align with `vitejs/vite#20163` (#5034) by @shulaoda
- rust/advanced-chunks: support `MatchGroup#name` to be dynamic (#5033) by @hyf0
- rolldown_plugin_build_import_analysis: align with `vitejs/vite#20117` (#5027) by @shulaoda
- rolldown_plugin_build_import_analysis: align with `vitejs/vite#20115` (#5020) by @shulaoda
- add validation warning for advanced chunks options without groups (#5009) by @sapphi-red

### 🐛 Bug Fixes

- moduleInfo is not updated when entry module is emitted by this.emitFile (#5032) by @IWANABETHATGUY
- preserveEntrySignatures: false generates circular imports that hangs with TLA (#5029) by @IWANABETHATGUY
- rolldown_plugin_build_import_analysis: align pure dynamic import handling with rolldown-vite (#5016) by @shulaoda
- plugin/vite-resolve: normalize leading slash (#5013) by @sapphi-red
- debug: `build_id` doesn't increase (#5015) by @hyf0
- side effects in this.emitFile({ type: 'chunk' }) is removed when preserveEntrySignatures: false is set (#5012) by @IWANABETHATGUY

### 🚜 Refactor

- rolldown_utils: simplify `block_on_spawn_all` (#5019) by @shulaoda
- use `rolldown_utils::futures::block_on` for `WatcherImpl#start` (#5018) by @shulaoda

### 📚 Documentation

- jsdoc: document `experimental.attachDebugInfo` (#5028) by @hyf0
- clarify that `advancedChunks` options are in bytes (#5022) by @sapphi-red
- add a note that sequential conversion may break the code (#5024) by @sapphi-red

### ⚙️ Miscellaneous Tasks

- infra: clean up `dist` before building `rolldown` (#5036) by @hyf0
- Align status notice in readme with documentation (#5021) by @rijkvanzanten

### ❤️ New Contributors

* @rijkvanzanten made their first contribution in [#5021](https://github.com/rolldown/rolldown/pull/5021)


## [1.0.0-beta.18] - 2025-06-19

### 🚀 Features

- advancedChunks: no need to include dependencies for `PreserveEntrySignatures::False` too (#5005) by @hyf0
- analyze commonjs module side effects and remove unused module (#5003) by @IWANABETHATGUY
- specially handling commonjs export stmt side effects (#5002) by @IWANABETHATGUY
- rolldown: oxc v0.73.2 (#5000) by @Boshen
- debug: attach both `session_id` and `build_id` to debug events (#4994) by @hyf0

### 🐛 Bug Fixes

- rolldown_plugin_import_glob: correctly handle alias glob patterns (#5007) by @shulaoda
- rolldown_plugin_alias: correctly handle `resolved_id` (#5006) by @shulaoda
- imported namespace is missing after external import namespace merging (#4999) by @IWANABETHATGUY
- node: add `experimental.viteMode` to option validator (#4996) by @sapphi-red
- plugin/vite-resolve: optional peer dep id parse error (#4995) by @sapphi-red

### 🚜 Refactor

- debug: remove extra mechanism for passing `session_id` and unnecessary passing of spans (#5001) by @hyf0

### 📚 Documentation

- update description about directive handling (#4992) by @IWANABETHATGUY

### ⚙️ Miscellaneous Tasks

- deps: update dependency rolldown-plugin-dts to v0.13.11 (#4863) by @renovate[bot]
- infra: update changelog configuration in `cliff.toml` (#4961) by @shulaoda
- infra: add knip to ci (#4991) by @webpro


## [1.0.0-beta.17] - 2025-06-17

### 🚀 Features

- binding: feat(binding): always use the same `BindingBundler` to create `BindingBundlerImpl` for the same `RolldownBuild` by @hyf0 in [#4985](https://github.com/rolldown/rolldown/pull/4985)
- debug: feat(debug): cache large string by @hyf0 in [#4882](https://github.com/rolldown/rolldown/pull/4882)
- feat: make require('./foo.json') work as expected even if the json is transformed to JS by a custom plugin by @IWANABETHATGUY in [#4984](https://github.com/rolldown/rolldown/pull/4984)
- feat: add diagnostic for multiple entries with UMD/IIFE formats by @shulaoda in [#4954](https://github.com/rolldown/rolldown/pull/4954)

### 🐛 Bug Fixes

- plugin/vite-resolve: fix(plugin/vite-resolve): importing an optional peer dep should throw an runtime error by @sapphi-red in [#4980](https://github.com/rolldown/rolldown/pull/4980)
- fix: upgrade napi, fix wasm url by @sxzz in [#4958](https://github.com/rolldown/rolldown/pull/4958)
- fix: should rewrite hmr related ast after test if `experimental.hmr` is enabled. by @IWANABETHATGUY in [#4972](https://github.com/rolldown/rolldown/pull/4972)

### 💼 Other

- build: provide `"type": "module"` in `package.json` by @iiio2 in [#4974](https://github.com/rolldown/rolldown/pull/4974)

### 🚜 Refactor

- binding: refactor(binding): rename `Bundler` to `BindingBundleImpl` by @hyf0 in [#4983](https://github.com/rolldown/rolldown/pull/4983)
- rolldown: refactor(rolldown): migrate to ESM package by @sxzz in [#4959](https://github.com/rolldown/rolldown/pull/4959)

### 📚 Documentation

- docs: explain how to transpile legacy decorators by @hyf0 in [#4989](https://github.com/rolldown/rolldown/pull/4989)
- code-splitting: docs(code-splitting): make terms bold instead of wrapped with inline code block by @hyf0 in [#4975](https://github.com/rolldown/rolldown/pull/4975)
- docs: improve advanced chunks by @hyf0 in [#4966](https://github.com/rolldown/rolldown/pull/4966)
- docs: refine code splitting guide by @sapphi-red in [#4969](https://github.com/rolldown/rolldown/pull/4969)

### 🧪 Testing

- test: place empty `package.json` to prevent all files to be treated as node mode by @sapphi-red in [#4979](https://github.com/rolldown/rolldown/pull/4979)

### ⚙️ Miscellaneous Tasks

- infra: chore(infra): ensure `pnpm install` for common commands by @hyf0 in [#4987](https://github.com/rolldown/rolldown/pull/4987)
- browser: chore(browser): add `@oxc-project/runtime` by @shulaoda in [#4988](https://github.com/rolldown/rolldown/pull/4988)
- deps: chore(deps): update dependency tsdown to v0.12.8 by @renovate[bot] in [#4977](https://github.com/rolldown/rolldown/pull/4977)
- deps: chore(deps): update npm packages (major) by @renovate[bot] in [#4963](https://github.com/rolldown/rolldown/pull/4963)
- deps: chore(deps): lock file maintenance npm packages by @renovate[bot] in [#4967](https://github.com/rolldown/rolldown/pull/4967)
- deps: chore(deps): lock file maintenance rust crates by @renovate[bot] in [#4965](https://github.com/rolldown/rolldown/pull/4965)
- deps: chore(deps): lock file maintenance npm packages by @renovate[bot] in [#4964](https://github.com/rolldown/rolldown/pull/4964)
- deps: chore(deps): update github-actions by @renovate[bot] in [#4962](https://github.com/rolldown/rolldown/pull/4962)

### ❤️ New Contributors

* @iiio2 made their first contribution in [#4974](https://github.com/rolldown/rolldown/pull/4974)


## [1.0.0-beta.16] - 2025-06-15

### 🚀 Features

- rolldown: feat(rolldown): oxc v0.73.0 by @Boshen in [#4948](https://github.com/rolldown/rolldown/pull/4948)
- types: feat(types): generate HMR runtime types by @sapphi-red in [#4928](https://github.com/rolldown/rolldown/pull/4928)
- rolldown_plugin_build_import_analysis: feat(rolldown_plugin_build_import_analysis): align with rolldown-vite by @shulaoda in [#4931](https://github.com/rolldown/rolldown/pull/4931)
- feat: allow to configure`experimental#attachDebugInfo: 'none'` to disable generating `#region` comments by @TheAlexLichter in [#4918](https://github.com/rolldown/rolldown/pull/4918)

### 🐛 Bug Fixes

- fix: deduplicate entry point by @IWANABETHATGUY in [#4956](https://github.com/rolldown/rolldown/pull/4956)
- fix: diagnostic message on minified code is too long to be readable by @IWANABETHATGUY in [#4950](https://github.com/rolldown/rolldown/pull/4950)
- fix: deduplicated entry points when have same `id`, `name`, and `filename` by @IWANABETHATGUY in [#4947](https://github.com/rolldown/rolldown/pull/4947)
- fix: unbundle does not work with outExtensions by @IWANABETHATGUY in [#4945](https://github.com/rolldown/rolldown/pull/4945)
- types: fix(types): remove `undefined` from return type of `generateHmrPatch` and `hmrInvalidate` by @sapphi-red in [#4943](https://github.com/rolldown/rolldown/pull/4943)
- fix: handle error in `hmrInvalidate` by @sapphi-red in [#4942](https://github.com/rolldown/rolldown/pull/4942)
- rolldown_plugin_build_import_analysis: fix(rolldown_plugin_build_import_analysis): preload helper is imported even if it’s not needed by @shulaoda in [#4936](https://github.com/rolldown/rolldown/pull/4936)
- hmr: fix(hmr): avoid panic after syntax error by @sapphi-red in [#4898](https://github.com/rolldown/rolldown/pull/4898)
- strictExecutionOrder: fix(strictExecutionOrder): ensure initialization chain of entry exports by @hyf0 in [#4933](https://github.com/rolldown/rolldown/pull/4933)
- fix: should not panic when meet a syntax error in watch mode. by @IWANABETHATGUY in [#4926](https://github.com/rolldown/rolldown/pull/4926)
- fix: regression, id filter normalization for windows is not working by @IWANABETHATGUY in [#4922](https://github.com/rolldown/rolldown/pull/4922)
- rolldown_plugin_json: fix(rolldown_plugin_json): incorrect default JSON export by @shulaoda in [#4924](https://github.com/rolldown/rolldown/pull/4924)

### 🚜 Refactor

- refactor: avoid unnecessary collect by @IWANABETHATGUY in [#4949](https://github.com/rolldown/rolldown/pull/4949)
- refactor: get normalized options via binding by @IWANABETHATGUY in [#4938](https://github.com/rolldown/rolldown/pull/4938)
- refactor: include DevRuntime base class even if custom HMR implementation is passed by @sapphi-red in [#4929](https://github.com/rolldown/rolldown/pull/4929)

### 📚 Documentation

- docs: clarify minifier state by @TheAlexLichter in [#4953](https://github.com/rolldown/rolldown/pull/4953)
- docs: drafting code splitting by @hyf0 in [#4951](https://github.com/rolldown/rolldown/pull/4951)
- docs: add link to repl by @sxzz in [#4917](https://github.com/rolldown/rolldown/pull/4917)
- pluginutils: docs(pluginutils): add example to simple filter functions by @sapphi-red in [#4915](https://github.com/rolldown/rolldown/pull/4915)

### 🧪 Testing

- test: add testcase for normalized output options by @IWANABETHATGUY in [#4939](https://github.com/rolldown/rolldown/pull/4939)
- hmr: test(hmr): enable `expectExecuted` for `generatePatchError` test by @sapphi-red in [#4937](https://github.com/rolldown/rolldown/pull/4937)
- test: evaluate hmr tests in node by @sapphi-red in [#4930](https://github.com/rolldown/rolldown/pull/4930)
- test: add "non used export" hmr test by @sapphi-red in [#4900](https://github.com/rolldown/rolldown/pull/4900)
- test: enable expectExecuted in HMR tests by @sapphi-red in [#4927](https://github.com/rolldown/rolldown/pull/4927)

### ⚙️ Miscellaneous Tasks

- deps: chore(deps): lock file maintenance npm packages by @renovate[bot] in [#4866](https://github.com/rolldown/rolldown/pull/4866)
- infra: chore(infra): remove comments related to `cspell` by @shulaoda in [#4940](https://github.com/rolldown/rolldown/pull/4940)
- ci: remove cargo bench test by @IWANABETHATGUY in [#4925](https://github.com/rolldown/rolldown/pull/4925)


## [1.0.0-beta.15] - 2025-06-11

### 🚀 Features

- feat: make native json plugin callable by @shulaoda in [#4908](https://github.com/rolldown/rolldown/pull/4908)
- feat: support preserveSignature in `this.emitFile` by @IWANABETHATGUY in [#4895](https://github.com/rolldown/rolldown/pull/4895)
- rolldown_plugin_module_preload_polyfill: feat(rolldown_plugin_module_preload_polyfill): expose plugin config by @shulaoda in [#4896](https://github.com/rolldown/rolldown/pull/4896)

### 🐛 Bug Fixes

- fix: add missing pathe polyfill by @sxzz in [#4912](https://github.com/rolldown/rolldown/pull/4912)
- node: fix(node): call options hook for experimental_scan by @sapphi-red in [#4911](https://github.com/rolldown/rolldown/pull/4911)
- fix: better `NoEntryPoint` error message by @IWANABETHATGUY in [#4903](https://github.com/rolldown/rolldown/pull/4903)
- fix: fail to bundle when provide entryPoint dynamiclly by @IWANABETHATGUY in [#4902](https://github.com/rolldown/rolldown/pull/4902)

### 🧪 Testing

- pluginutils: test(pluginutils): add query suffix case for `makeIdFiltersToMatchWithQuery` by @sapphi-red in [#4914](https://github.com/rolldown/rolldown/pull/4914)
- test: copy rollup preserveEntrySignature override-via-plugin test by @IWANABETHATGUY in [#4910](https://github.com/rolldown/rolldown/pull/4910)
- test: include error in hmr test snapshots by @sapphi-red in [#4899](https://github.com/rolldown/rolldown/pull/4899)

### ⚙️ Miscellaneous Tasks

- chore: update committed dts by @sapphi-red in [#4913](https://github.com/rolldown/rolldown/pull/4913)
- chore: bump rolldown-ariadne by @IWANABETHATGUY in [#4909](https://github.com/rolldown/rolldown/pull/4909)
- improve prepare release commit message by @Boshen


## [1.0.0-beta.14] - 2025-06-10

### 💥 BREAKING CHANGES

- refactor!: move preserveEntrySignatures from output to input options by @IWANABETHATGUY in [#4871](https://github.com/rolldown/rolldown/pull/4871)

### 🚀 Features

- strictExecutionOrder: feat(strictExecutionOrder): no need to generate plain chunk imports for addressing side effects by @hyf0 in [#4890](https://github.com/rolldown/rolldown/pull/4890)
- feat: make wasm fallback plugin callable in node by @shulaoda in [#4885](https://github.com/rolldown/rolldown/pull/4885)
- rolldown_plugin_module_preload_polyfill: feat(rolldown_plugin_module_preload_polyfill): align with `rolldown-vite` by @shulaoda in [#4884](https://github.com/rolldown/rolldown/pull/4884)
- attachDebugInfo: feat(attachDebugInfo): improve format and distinguish dynamic entries by @hyf0 in [#4861](https://github.com/rolldown/rolldown/pull/4861)

### 🐛 Bug Fixes

- binding: fix(binding): preserveModules infinity loop by @Brooooooklyn in [#4879](https://github.com/rolldown/rolldown/pull/4879)
- fix: dynamic import does not return exports when preserveEntrySignatures: false is set and the importee is an entry by @IWANABETHATGUY in [#4891](https://github.com/rolldown/rolldown/pull/4891)
- fix: preserveEntrySignatures panics with an edge case by @IWANABETHATGUY in [#4877](https://github.com/rolldown/rolldown/pull/4877)
- fix: directives are missing with `preserveModules` by @IWANABETHATGUY in [#4876](https://github.com/rolldown/rolldown/pull/4876)
- fix: exports from dynamically imported chunks are removed by @IWANABETHATGUY in [#4875](https://github.com/rolldown/rolldown/pull/4875)

### 🚜 Refactor

- refactor: clearify Chunk entry kind by @IWANABETHATGUY in [#4892](https://github.com/rolldown/rolldown/pull/4892)
- rolldown_plugin_build_import_analysis: refactor(rolldown_plugin_build_import_analysis): align the logic with `rolldown-vite` by @shulaoda in [#4856](https://github.com/rolldown/rolldown/pull/4856)

### 📚 Documentation

- contrib-guide: docs(contrib-guide): add HMR testing section by @sapphi-red in [#4888](https://github.com/rolldown/rolldown/pull/4888)
- guide: docs(guide): update release channels - remove nightly, add pkg.pr.new by @Boshen in [#4881](https://github.com/rolldown/rolldown/pull/4881)

### 🧪 Testing

- test: rename `rolldown/topics/hmr/mutiply-entires` to snake case by @sapphi-red in [#4889](https://github.com/rolldown/rolldown/pull/4889)
- test: add test for #4849 by @sapphi-red in [#4887](https://github.com/rolldown/rolldown/pull/4887)
- test: setup infra for hmr tests by @sapphi-red in [#4886](https://github.com/rolldown/rolldown/pull/4886)

### ⚙️ Miscellaneous Tasks

- ci: disable publish to nightly by @Boshen in [#4874](https://github.com/rolldown/rolldown/pull/4874)
- ci: extend Vite tests timeout by @sapphi-red in [#4870](https://github.com/rolldown/rolldown/pull/4870)
- fix publish-to-npm.yml by @Boshen

### ◀️ Revert

- revert: "refactor(rolldown_plugin_load_fallback): align with rolldown-vite" by @shulaoda in [#4868](https://github.com/rolldown/rolldown/pull/4868)


## [1.0.0-beta.13] - 2025-06-07

### 🚀 Features

- feat: merge same `ImportNamespaceSpecifier`  for external module by @IWANABETHATGUY in [#4373](https://github.com/rolldown/rolldown/pull/4373)

### 🐛 Bug Fixes

- ci: disable `generate_release_notes` by @Boshen

### 📚 Documentation

- docs: add MAINTENANCE.md; remove rolldown.rs/contrib-guide/release by @Boshen in [#4854](https://github.com/rolldown/rolldown/pull/4854)

### ⚙️ Miscellaneous Tasks

- add input description to prepare-release.yml by @Boshen
- fix prepare-release.yml by @Boshen
- CHANGELOG.md: ci(CHANGELOG.md): use git-cliff to generate changelogs by @Boshen in [#4858](https://github.com/rolldown/rolldown/pull/4858)

## [1.0.0-beta.12](https://github.com/rolldown/rolldown/compare/v1.0.0-beta.11...v1.0.0-beta.12) (2025-06-06)

### Features

* `preseveEntrySignature: allow-extension` ([#4800](https://github.com/rolldown/rolldown/issues/4800)) ([83d4d62](https://github.com/rolldown/rolldown/commit/83d4d62121182f3f9f54193d722a73b1dd8290de))
* **advancedChunks:** only move the captured module itself if `preserveEntrySignatures` is `allow-extension` ([#4847](https://github.com/rolldown/rolldown/issues/4847)) ([57e7f8b](https://github.com/rolldown/rolldown/commit/57e7f8b72bb418cc966c1724d74a820a6433020f))
* interop `import('some-cjs-module.js')` ([#4816](https://github.com/rolldown/rolldown/issues/4816)) ([b9ac5f0](https://github.com/rolldown/rolldown/commit/b9ac5f03d681dd5e92a71920330506218f2dea65))
* **rolldown:** oxc v0.72.3 ([#4845](https://github.com/rolldown/rolldown/issues/4845)) ([759525d](https://github.com/rolldown/rolldown/commit/759525d8ef0c674df66e1229413709ce67564287)), closes [#4796](https://github.com/rolldown/rolldown/issues/4796) [#4846](https://github.com/rolldown/rolldown/issues/4846)
* support preserveEntrySignatures: exports-only ([#4848](https://github.com/rolldown/rolldown/issues/4848)) ([63dc546](https://github.com/rolldown/rolldown/commit/63dc54624124b98fe5ad04c66eb0e0605bb8e852))
* support preserveEntrySignatures: false ([#4844](https://github.com/rolldown/rolldown/issues/4844)) ([e4e90e9](https://github.com/rolldown/rolldown/commit/e4e90e9373e8a1e9b60003a016d57907cc88b188))
* take node esm spec into account for dynamic imports in cjs modules that satisfy nodejs ([#4819](https://github.com/rolldown/rolldown/issues/4819)) ([640af3a](https://github.com/rolldown/rolldown/commit/640af3aac4654379766e035ebc420b59536d3281)), closes [#4289](https://github.com/rolldown/rolldown/issues/4289)

### Bug Fixes

* 4818 ([#4828](https://github.com/rolldown/rolldown/issues/4828)) ([c6aaebe](https://github.com/rolldown/rolldown/commit/c6aaebef4772830efb90302847760ebca18a3f60)), closes [#4818](https://github.com/rolldown/rolldown/issues/4818)
* **bindings:** cargo cache with napi typedef issues ([#4820](https://github.com/rolldown/rolldown/issues/4820)) ([a733205](https://github.com/rolldown/rolldown/commit/a7332057c88f054c7fe93b1123b4b33f17a501e5))
* **bindings:** update @napi-rs/cli ([#4830](https://github.com/rolldown/rolldown/issues/4830)) ([052fb45](https://github.com/rolldown/rolldown/commit/052fb458abeb71830920a0c7bc3db96a08a44355))
* **ci:** fix prepare-release permission ([b04b74e](https://github.com/rolldown/rolldown/commit/b04b74e6af4c6ed2744cce636f228775d81a47e6))
* cjs namespace property optimization access should only apply to ns_alias prop `default` ([#4836](https://github.com/rolldown/rolldown/issues/4836)) ([d811ec5](https://github.com/rolldown/rolldown/commit/d811ec5d9e83f28c059c196ed688c8b0c217b2da))
* handle error in `generateHmrPatch` ([#4837](https://github.com/rolldown/rolldown/issues/4837)) ([4a9af95](https://github.com/rolldown/rolldown/commit/4a9af950feacfb6a628ef1faa51369aef57d3c42))
* **hmr:** generate unique import binding ([#4849](https://github.com/rolldown/rolldown/issues/4849)) ([56a2214](https://github.com/rolldown/rolldown/commit/56a22145c32c9eb963dc4aeca3c0b0b88a513145))
* Preserve default export for preserveModules ([#4814](https://github.com/rolldown/rolldown/issues/4814)) ([b30ab1f](https://github.com/rolldown/rolldown/commit/b30ab1f6899640ff61421ef81ed5d19f76e5ce06)), closes [#4758](https://github.com/rolldown/rolldown/issues/4758)
* regression with namespace import ([#4825](https://github.com/rolldown/rolldown/issues/4825)) ([f051675](https://github.com/rolldown/rolldown/commit/f0516754de1d3ef107247255f7b8511444f76d5a)), closes [#4824](https://github.com/rolldown/rolldown/issues/4824)
* **rolldown:** fix double initialization of tracing subscriber ([#4831](https://github.com/rolldown/rolldown/issues/4831)) ([ac2f6bf](https://github.com/rolldown/rolldown/commit/ac2f6bf2eaf54f205a1074ed636d6f3a8e742929))
* should leave all file extensions (even double) unchanged. ([#4822](https://github.com/rolldown/rolldown/issues/4822)) ([9abc457](https://github.com/rolldown/rolldown/commit/9abc457f4e8e571267b2c59f8b7d5d5902c1c455)), closes [#4702](https://github.com/rolldown/rolldown/issues/4702)
## [1.0.0-beta.11](https://github.com/rolldown/rolldown/compare/v1.0.0-beta.10...v1.0.0-beta.11) (2025-06-04)

### Features

* add `index_of_one` utils for `Bitset` ([#4779](https://github.com/rolldown/rolldown/issues/4779)) ([16aff17](https://github.com/rolldown/rolldown/commit/16aff17677e8afa62a829dbda40c6c34181093f2))
* **advancedChunks:** support function for `output.advancedChunks.groups[].test` ([#4644](https://github.com/rolldown/rolldown/issues/4644)) ([0a177d4](https://github.com/rolldown/rolldown/commit/0a177d4fcdc6a135d100462233cdfa9c17c2503f)), closes [#4477](https://github.com/rolldown/rolldown/issues/4477)
* **node:** support Rollup-style JSX options ([#4756](https://github.com/rolldown/rolldown/issues/4756)) ([2c4c2a8](https://github.com/rolldown/rolldown/commit/2c4c2a8188e36ddcd9cac4bd5d5cf60f96566edc)), closes [#4752](https://github.com/rolldown/rolldown/issues/4752)
* **rolldown_plugin_isolated_declaration:** improve diagnostic messages ([#4731](https://github.com/rolldown/rolldown/issues/4731)) ([87188ed](https://github.com/rolldown/rolldown/commit/87188edbcf5520a2a8552b870f348109c8c41875)), closes [#4040](https://github.com/rolldown/rolldown/issues/4040)
* warn when assignment to bundle is detected ([#4792](https://github.com/rolldown/rolldown/issues/4792)) ([833c4e0](https://github.com/rolldown/rolldown/commit/833c4e0c7cfe8cc30fd590302a2efcd49d8fcb8c))

### Bug Fixes

* allow user call `resolveId` hook in deps of internal runtime module ([#4733](https://github.com/rolldown/rolldown/issues/4733)) ([1995519](https://github.com/rolldown/rolldown/commit/1995519633a5602400b89f3c1bf4d94f08ab6d68))
* **browser:** fix binding types ([#4488](https://github.com/rolldown/rolldown/issues/4488)) ([287a573](https://github.com/rolldown/rolldown/commit/287a57391a7115a726cb1cfb0dec624153f33ccc)), closes [#4753](https://github.com/rolldown/rolldown/issues/4753) [#4754](https://github.com/rolldown/rolldown/issues/4754) [#4724](https://github.com/rolldown/rolldown/issues/4724)
* built file references undeclared import_foo$n ([#4745](https://github.com/rolldown/rolldown/issues/4745)) ([cb30e40](https://github.com/rolldown/rolldown/commit/cb30e400570dc1100c0ef983633119b3034ae699)), closes [#4740](https://github.com/rolldown/rolldown/issues/4740)
* cjs namespace access property optimization ([#4803](https://github.com/rolldown/rolldown/issues/4803)) ([08bf380](https://github.com/rolldown/rolldown/commit/08bf380f2673c44812984641d1bc354a0697ec53))
* computed property access is converted to static property access when using namespace import ([#4781](https://github.com/rolldown/rolldown/issues/4781)) ([e692385](https://github.com/rolldown/rolldown/commit/e69238501a705cd1e08bbe47559df7bfe1b6378f))
* include side-effect modules in preserveModules mode ([#4710](https://github.com/rolldown/rolldown/issues/4710)) ([ac4e5db](https://github.com/rolldown/rolldown/commit/ac4e5db3df010c44b3e85337660e66c050bb7157))
* **infra:** fails to load wasm fallback on WebContainer ([#4770](https://github.com/rolldown/rolldown/issues/4770)) ([5cb498e](https://github.com/rolldown/rolldown/commit/5cb498ea139e066c7b9ed2b1333e33aea1ae11e0)), closes [#4762](https://github.com/rolldown/rolldown/issues/4762)
* **inlineDynamicImports:** ensure tla module execution correctly ([#4764](https://github.com/rolldown/rolldown/issues/4764)) ([92851ea](https://github.com/rolldown/rolldown/commit/92851eaedf3e2d5eaa612ce6634dac2878a35bc7)), closes [#4610](https://github.com/rolldown/rolldown/issues/4610)
* **package.json:** fix husky not working ([98c54a1](https://github.com/rolldown/rolldown/commit/98c54a11839a6480d30c9ddcf0ab04fc8b245dc0))
* renamed exports when enabled preserveModules ([#4728](https://github.com/rolldown/rolldown/issues/4728)) ([4da8973](https://github.com/rolldown/rolldown/commit/4da89732d145a8552a2cc01034c6ca12ae6fc90f)), closes [#4698](https://github.com/rolldown/rolldown/issues/4698)
* **rolldown_plugin_build_import_analysis:** align `load` hook ([#4742](https://github.com/rolldown/rolldown/issues/4742)) ([10f8e10](https://github.com/rolldown/rolldown/commit/10f8e10afd5f8c057926ef504a598ef84af218bf)), closes [#3983](https://github.com/rolldown/rolldown/issues/3983)
* test napi-derive file lock ([#4751](https://github.com/rolldown/rolldown/issues/4751)) ([2203f74](https://github.com/rolldown/rolldown/commit/2203f74b8055464b8f450307b3372ce97c1b910d))
## [1.0.0-beta.10](https://github.com/rolldown/rolldown/compare/v1.0.0-beta.9...v1.0.0-beta.10) (2025-05-28)

### ⚠ BREAKING CHANGES

* "feat!: always make the default value of `platform` as `browser`" (#4720)
* **resolve:** add `module` to resolve conditions by default (#4703)
* **types/resolve:** use literal `'import-statement'` to refer to static import statements instead of `'import'` (#4689)
* **rust:** merge `target` option into `transform` (#4665)
* **rust:** merge `jsx` option into `transform` (#4654)
* always make the default value of `platform` as `browser` (#4611)

### Features

* add preserveEntrySignatures option ([#4640](https://github.com/rolldown/rolldown/issues/4640)) ([3979e2a](https://github.com/rolldown/rolldown/commit/3979e2a5a1e89482a2b1c287fbdc42b238db18c6))
* always make the default value of `platform` as `browser` ([#4611](https://github.com/rolldown/rolldown/issues/4611)) ([43425a0](https://github.com/rolldown/rolldown/commit/43425a01b4a5549f2b47746aff2385fb23f2bfd0))
* **binding:** preserveModulesRoot ([#4631](https://github.com/rolldown/rolldown/issues/4631)) ([51df2b7](https://github.com/rolldown/rolldown/commit/51df2b737e941084ede9763b9e0d061bb2fc78d3))
* **ci:** apply `cargo shear --fix` correctly in autofix ([#4677](https://github.com/rolldown/rolldown/issues/4677)) ([15934eb](https://github.com/rolldown/rolldown/commit/15934eb25e0db4d73cccd2e9418a828f766e2073))
* **debug:** add `StmtSideEffect` to record the specific reason why a stmt has side effect ([#4671](https://github.com/rolldown/rolldown/issues/4671)) ([5982115](https://github.com/rolldown/rolldown/commit/5982115ae4db2851f231f30c74aadbe5e8a2c2c1))
* **debug:** emit more details for `Module[#imports](https://github.com/rolldown/rolldown/issues/imports)` ([#4619](https://github.com/rolldown/rolldown/issues/4619)) ([42afd5a](https://github.com/rolldown/rolldown/commit/42afd5aa4f8f8edab142a4b42a277854a4f9a1dd))
* enable cjs ns merge for react ([#4621](https://github.com/rolldown/rolldown/issues/4621)) ([ddf8da8](https://github.com/rolldown/rolldown/commit/ddf8da885f6999a68bad876df4b1a6db2a6930d4))
* enable loose mode validate option and give warning ([#4638](https://github.com/rolldown/rolldown/issues/4638)) ([080ce4e](https://github.com/rolldown/rolldown/commit/080ce4eedaf5803a4da09a84e1f9c18498c38df4))
* **on_demand_wrapping:** don't wrap modules that don't rely on others and have side effect ([#4670](https://github.com/rolldown/rolldown/issues/4670)) ([326e6f5](https://github.com/rolldown/rolldown/commit/326e6f5bd17cac7ef68173d80cb19b67e521bd93))
* **plugin/json:** only transform module with `moduleType: 'json'` ([#4554](https://github.com/rolldown/rolldown/issues/4554)) ([c69e332](https://github.com/rolldown/rolldown/commit/c69e33224e2f41701acf7fc5a861f398d8a4ddc2))
* **rolldown_plugin_oxc_runtime:** support automatic resolution of oxc runtime helpers ([#4641](https://github.com/rolldown/rolldown/issues/4641)) ([b09c035](https://github.com/rolldown/rolldown/commit/b09c0350d91cb7da1f5d40a53a94a5300fba5118)), closes [#4597](https://github.com/rolldown/rolldown/issues/4597)
* **rolldown_plugin_utils:** extract `check_public_file` from `rolldown_plugin_asset` ([#4660](https://github.com/rolldown/rolldown/issues/4660)) ([1b3b8c3](https://github.com/rolldown/rolldown/commit/1b3b8c3e757795a58f709e4a4b57f9fa6a6aafe9))
* **rolldown_plugin_utils:** extract `file_to_url` from `rolldown_plugin_asset` ([#4663](https://github.com/rolldown/rolldown/issues/4663)) ([40290b5](https://github.com/rolldown/rolldown/commit/40290b573e7434bfe59be1aaff5ec18b5cd896c9))
* **rolldown_plugin_utils:** support `create_to_import_meta_url_based_relative_runtime` ([#4657](https://github.com/rolldown/rolldown/issues/4657)) ([d77354f](https://github.com/rolldown/rolldown/commit/d77354f590ee89015bb5c042d29a489b7f37b81a))
* **rolldown_plugin_utils:** support `to_output_file_path_in_js` ([#4659](https://github.com/rolldown/rolldown/issues/4659)) ([9e8c677](https://github.com/rolldown/rolldown/commit/9e8c677d1a60ef8a5198f473dcf44a32f03dbf20))
* **rolldown:** oxc v0.71.0 and napi beta ([#4618](https://github.com/rolldown/rolldown/issues/4618)) ([76c39c6](https://github.com/rolldown/rolldown/commit/76c39c68177af95243714e375536728ee3de06f4)), closes [#4614](https://github.com/rolldown/rolldown/issues/4614)
* **rolldown:** oxc v0.72.0 ([#4658](https://github.com/rolldown/rolldown/issues/4658)) ([8371a90](https://github.com/rolldown/rolldown/commit/8371a909a6d27c4ba7ea45adf8ba6982f70131ff))
* **rolldown:** oxc v0.72.1 and oxc-resolver v11 ([#4718](https://github.com/rolldown/rolldown/issues/4718)) ([79a47fc](https://github.com/rolldown/rolldown/commit/79a47fcda9dd236bad3ba5edc489adc394a3ae89))
* **rust:** merge `jsx` option into `transform` ([#4654](https://github.com/rolldown/rolldown/issues/4654)) ([4872097](https://github.com/rolldown/rolldown/commit/48720977fa6ff5a1a42522141a779edbef025e14)), closes [#4447](https://github.com/rolldown/rolldown/issues/4447)
* **rust:** merge `target` option into `transform` ([#4665](https://github.com/rolldown/rolldown/issues/4665)) ([f9aa33a](https://github.com/rolldown/rolldown/commit/f9aa33a83915020468b9901cc9703003389e0979)), closes [#4651](https://github.com/rolldown/rolldown/issues/4651)
* **rust:** preserveModulesRoot ([#4630](https://github.com/rolldown/rolldown/issues/4630)) ([9f62c77](https://github.com/rolldown/rolldown/commit/9f62c772f7d55d92b94d6ff4f74c8d50e4297e67))
* support to attach chunk debug information in output ([#4633](https://github.com/rolldown/rolldown/issues/4633)) ([1079582](https://github.com/rolldown/rolldown/commit/10795828dba9cf129c5764381017b766e6cbc9ef))

### Bug Fixes

* `chunk.name` is not sanitized when the chunk is a common chunk ([#4712](https://github.com/rolldown/rolldown/issues/4712)) ([5949f2a](https://github.com/rolldown/rolldown/commit/5949f2a3307192a44b534f6c700dae6f78e84b48)), closes [#4709](https://github.com/rolldown/rolldown/issues/4709)
* `preserveModules` co exists with multi entrypoints ([#4626](https://github.com/rolldown/rolldown/issues/4626)) ([b46bdea](https://github.com/rolldown/rolldown/commit/b46bdea5d5e80bf82d3e4f94eda0260b6c71c4af)), closes [#4624](https://github.com/rolldown/rolldown/issues/4624)
* `preserveModules` don't respect relative path ([#4629](https://github.com/rolldown/rolldown/issues/4629)) ([9727493](https://github.com/rolldown/rolldown/commit/972749302ca84d2317d261f88eedd1835d866bb6))
* **browser:** disable oxc runtime plugin ([#4708](https://github.com/rolldown/rolldown/issues/4708)) ([0ec9e7d](https://github.com/rolldown/rolldown/commit/0ec9e7ddef4df61f017800308be9226426ffef16)), closes [#4707](https://github.com/rolldown/rolldown/issues/4707)
* **ci:** add missing `just` in warmup workflow ([c6a1eb1](https://github.com/rolldown/rolldown/commit/c6a1eb110f48dbca6b06be7bd83bb9e6fc15de24))
* **ci:** cache release builds to a different cache key ([a002b24](https://github.com/rolldown/rolldown/commit/a002b2406f2b27065f093e2f6cad407fa82438dd))
* **ci:** cache warmup should include build artifacts ([#4675](https://github.com/rolldown/rolldown/issues/4675)) ([0f2c3b5](https://github.com/rolldown/rolldown/commit/0f2c3b5951123ff1c102df05abfa074ea53d36a5))
* **ci:** fix double pnpm cache from `setup-node` ([#4685](https://github.com/rolldown/rolldown/issues/4685)) ([db25788](https://github.com/rolldown/rolldown/commit/db25788e67c3cfaff8989ec467788eaa5444956c))
* **ci:** fix release build ([#4691](https://github.com/rolldown/rolldown/issues/4691)) ([82620b9](https://github.com/rolldown/rolldown/commit/82620b916544a35dab613141a78e45c598859637))
* **ci:** fix template-injection in benchmark-rust ([57b2792](https://github.com/rolldown/rolldown/commit/57b27922a3a9ad7c8f5a46335ce676a7d1f9a155))
* **ci:** fix template-injection in publish-to-npm-for-nightly-canary.yml ([352340f](https://github.com/rolldown/rolldown/commit/352340faa336226a526d993f3a393f7c2ed369c2))
* **ci:** in metrics, use cache instead of `cargo fetch` ([937eae9](https://github.com/rolldown/rolldown/commit/937eae93c25b0abd05f67ffda2f02b01216bf23d))
* **ci:** pnpm install --ignore-scripts ([#4696](https://github.com/rolldown/rolldown/issues/4696)) ([e040112](https://github.com/rolldown/rolldown/commit/e0401125b588f74dd9b4164292a83a41146594f1))
* cjs namespace merge in incremental build ([#4613](https://github.com/rolldown/rolldown/issues/4613)) ([b1c500e](https://github.com/rolldown/rolldown/commit/b1c500e11a9d8b8132e02027d43487e48b5a5e86))
* dynamic chunk not exported when using preserveModules ([#4650](https://github.com/rolldown/rolldown/issues/4650)) ([d91dfb5](https://github.com/rolldown/rolldown/commit/d91dfb553598b445875e495f9c5ad70ea9e5824c))
* exported dynamic import is treated as unused ([#4648](https://github.com/rolldown/rolldown/issues/4648)) ([edf5b4e](https://github.com/rolldown/rolldown/commit/edf5b4e1652a7b241ab7f214d29777593f4f8096)), closes [#4646](https://github.com/rolldown/rolldown/issues/4646)
* generated wrapper stmt info should be always included on demand ([#4639](https://github.com/rolldown/rolldown/issues/4639)) ([ed553e1](https://github.com/rolldown/rolldown/commit/ed553e1e4c8d5a88a3e103307c60c1b6eae2e228))
* index out of bounds panic at oxc_index-3.0.0 with recent canary version ([#4713](https://github.com/rolldown/rolldown/issues/4713)) ([3fad6f4](https://github.com/rolldown/rolldown/commit/3fad6f497b55345fe2c626692b6e8cc402a31967))
* **inlineDynamicImports:** ensure tla module execution correctly ([#4627](https://github.com/rolldown/rolldown/issues/4627)) ([e9ef28c](https://github.com/rolldown/rolldown/commit/e9ef28c1c54add876596a31edd8f4137484b6fca))
* partial cjs namespace merge with chunk split ([#4598](https://github.com/rolldown/rolldown/issues/4598)) ([83a0b40](https://github.com/rolldown/rolldown/commit/83a0b401873c1c57bc38f5f59b870163a9370371))
* partial merge cjs namespace ([#4595](https://github.com/rolldown/rolldown/issues/4595)) ([0085f4a](https://github.com/rolldown/rolldown/commit/0085f4a4d5d2d25cd33f58a430c591c2c05404a9))
* removing unused dynamic imported chunks ([#4655](https://github.com/rolldown/rolldown/issues/4655)) ([fe21db7](https://github.com/rolldown/rolldown/commit/fe21db75b99622d62d6bc3db85a99531418f0f5e))
* **resolve:** add `module` to resolve conditions by default ([#4703](https://github.com/rolldown/rolldown/issues/4703)) ([9727531](https://github.com/rolldown/rolldown/commit/97275318e6f37756509a8f185bf51759e597394f))
* **rolldown/browser:** auto-resolve oxc runtime helpers ([#4645](https://github.com/rolldown/rolldown/issues/4645)) ([701bbc3](https://github.com/rolldown/rolldown/commit/701bbc3d342e8821a3efef3bc6ef09b7709b475f)), closes [#4641](https://github.com/rolldown/rolldown/issues/4641)
* **rust:** avoid panic in `into_assignment_target` ([#4688](https://github.com/rolldown/rolldown/issues/4688)) ([6c806ea](https://github.com/rolldown/rolldown/commit/6c806ea2c4310f0b14563aa30573bb87cadc7e22)), closes [#4304](https://github.com/rolldown/rolldown/issues/4304)
* sort dynamic entires with topological order before tree shake. ([#4694](https://github.com/rolldown/rolldown/issues/4694)) ([bc96622](https://github.com/rolldown/rolldown/commit/bc966224d226f80da7a294fe9befba605f508f24))
* **strict_execution_order:** runtime module should not be wrapped by default ([#4692](https://github.com/rolldown/rolldown/issues/4692)) ([4008ac0](https://github.com/rolldown/rolldown/commit/4008ac0aa1172ade61174b6d0183e584f60f8b24))
* **strict_execution_order:** wrapped module should be included on demand ([#4687](https://github.com/rolldown/rolldown/issues/4687)) ([7c21036](https://github.com/rolldown/rolldown/commit/7c210364a7d2bf34f175714a8f015c230f3f054f))
* strip path for require identifier with `preserveModules` ([#4704](https://github.com/rolldown/rolldown/issues/4704)) ([2ba8e28](https://github.com/rolldown/rolldown/commit/2ba8e2836baa95074e1ba0d07622b867a22b9317)), closes [#4700](https://github.com/rolldown/rolldown/issues/4700)
* **types/resolve:** use literal `'import-statement'` to refer to static import statements instead of `'import'` ([#4689](https://github.com/rolldown/rolldown/issues/4689)) ([eee51d8](https://github.com/rolldown/rolldown/commit/eee51d8756d2d957445d804f5240a5461b93eba9))
* **watch:** watch linux path at windows ([#4625](https://github.com/rolldown/rolldown/issues/4625)) ([ed594aa](https://github.com/rolldown/rolldown/commit/ed594aa6b5ae83ff52ae8727568fefbc977971d4))

### Performance Improvements

* change `profile.dev.debug` to improve rust compile times ([#4623](https://github.com/rolldown/rolldown/issues/4623)) ([0312f66](https://github.com/rolldown/rolldown/commit/0312f66401f70a0de261c8a88953f1e6c3227a38))
* **rolldown:** prevent duplicate text embedding with `include_str!` ([#4664](https://github.com/rolldown/rolldown/issues/4664)) ([8fee5af](https://github.com/rolldown/rolldown/commit/8fee5afe0c6a7870e24f1395e62ca05001554a9b)), closes [#4354](https://github.com/rolldown/rolldown/issues/4354)

### Reverts

* "feat!: always make the default value of `platform` as `browser`" ([#4720](https://github.com/rolldown/rolldown/issues/4720)) ([4021325](https://github.com/rolldown/rolldown/commit/4021325f2b6a5fbf10ed8ed2bfcfa50f33781cfd)), closes [rolldown/rolldown#4611](https://github.com/rolldown/rolldown/issues/4611)
* "refactor(rolldown_sourcemap): remove unused `SourceJoiner[#prepend](https://github.com/rolldown/rolldown/issues/prepend)_source`" ([#4632](https://github.com/rolldown/rolldown/issues/4632)) ([719ec89](https://github.com/rolldown/rolldown/commit/719ec89f74e2dc1d86c4b3357ba26cdf36c9ecc4)), closes [rolldown/rolldown#4431](https://github.com/rolldown/rolldown/issues/4431)
* `feat(on_demand_wrapping): don't wrap modules that don't rely on others and have side effect [#4670](https://github.com/rolldown/rolldown/issues/4670)` ([#4686](https://github.com/rolldown/rolldown/issues/4686)) ([8a77ce9](https://github.com/rolldown/rolldown/commit/8a77ce9a7bc3f5dd3f3af56c90b836362b840a8b))
## [1.0.0-beta.9](https://github.com/rolldown/rolldown/compare/v1.0.0-beta.8...v1.0.0-beta.9) (2025-05-19)

### ⚠ BREAKING CHANGES

* **rolldown:** update to `oxc@0.70.0` to refine printing comments  (#4562)
* align directive rendering with esbuild (#4557)
* don't expose `And` class in pluginutils (#4537)
* add `OutputOptions#legalComments` and remove `OutputOptions#comments` (#4528)
* expose `withFilter` function via `rolldown/filter` instead of `rolldown` (#4369)
* allows users to specify filter expressions directly using an array syntax instead of an object with a "custom" key (#4368)
* use camel cases for `package.json#exports` field (#4366)
* filter expr binding conversion (#4343)

### Features

* add `OutputOptions[#legal](https://github.com/rolldown/rolldown/issues/legal)Comments` and remove `OutputOptions[#comments](https://github.com/rolldown/rolldown/issues/comments)` ([#4528](https://github.com/rolldown/rolldown/issues/4528)) ([aa7e253](https://github.com/rolldown/rolldown/commit/aa7e253b28627cd99ebb0af666a701c4888e6f76))
* add watcher.off method to remove event listener ([#4388](https://github.com/rolldown/rolldown/issues/4388)) ([ca23f2d](https://github.com/rolldown/rolldown/commit/ca23f2d5e6a7b93f8069cae7f3b9ebcfe6a6ade7)), closes [#4382](https://github.com/rolldown/rolldown/issues/4382)
* allow to preserve legal comments with `minify: true` ([#4591](https://github.com/rolldown/rolldown/issues/4591)) ([9a837fc](https://github.com/rolldown/rolldown/commit/9a837fc1cee917796c1dc748abd09236157d8561))
* allows users to specify filter expressions directly using an array syntax instead of an object with a "custom" key ([#4368](https://github.com/rolldown/rolldown/issues/4368)) ([4a50b6d](https://github.com/rolldown/rolldown/commit/4a50b6d0693b189a1193463b723001670147bf63))
* better error message for `manualChunks` ([#4530](https://github.com/rolldown/rolldown/issues/4530)) ([5b8c925](https://github.com/rolldown/rolldown/commit/5b8c925613d6b2033aa2b206bd082dba77b3dd69))
* **builtin-plugin:** expose `webWorkerPostPlugin` ([#4594](https://github.com/rolldown/rolldown/issues/4594)) ([d89470c](https://github.com/rolldown/rolldown/commit/d89470c050333df5e4398036cbf89243bdc1ba4a))
* cleanUrl rust interpreter ([#4480](https://github.com/rolldown/rolldown/issues/4480)) ([1988c3d](https://github.com/rolldown/rolldown/commit/1988c3dc7e5902d92294b46e9a7330d226853430))
* **debug:** add `call_id` to load and transform events ([#4296](https://github.com/rolldown/rolldown/issues/4296)) ([432acb3](https://github.com/rolldown/rolldown/commit/432acb39524da9e002f43875c097c11d25f52716))
* **debug:** distinguish if `resolve_id` hook is called automatically or manually ([#4254](https://github.com/rolldown/rolldown/issues/4254)) ([1b90f35](https://github.com/rolldown/rolldown/commit/1b90f357758f784dd47b6b279950fa34054d01d2))
* **debug:** emit `ModuleGraphReady` event ([#4515](https://github.com/rolldown/rolldown/issues/4515)) ([29d88da](https://github.com/rolldown/rolldown/commit/29d88dac633f3d6e25fbd690792d24d46b6fe6c4)), closes [#4135](https://github.com/rolldown/rolldown/issues/4135)
* **debug:** inject `call_id` for each pair of `HookResolveIdCall` events ([#4255](https://github.com/rolldown/rolldown/issues/4255)) ([d93e66e](https://github.com/rolldown/rolldown/commit/d93e66e2e719284d04357511a5da2073b26b8651))
* expose `isolatedDeclaration` ([#4410](https://github.com/rolldown/rolldown/issues/4410)) ([92d35c5](https://github.com/rolldown/rolldown/commit/92d35c5a03c9505f9aa4e121bb81a4581958428b))
* expose `loadConfig` in api ([#4428](https://github.com/rolldown/rolldown/issues/4428)) ([9a4338e](https://github.com/rolldown/rolldown/commit/9a4338e8fd596804b88b5caef71730aabaf2ba2b)), closes [#4313](https://github.com/rolldown/rolldown/issues/4313)
* expose oxc-resolver ([#4485](https://github.com/rolldown/rolldown/issues/4485)) ([8aae298](https://github.com/rolldown/rolldown/commit/8aae298a59d058a2ca1653bcb93b87ff957319c1))
* filter expr binding conversion ([#4343](https://github.com/rolldown/rolldown/issues/4343)) ([6c73e55](https://github.com/rolldown/rolldown/commit/6c73e55cb14bfbcd6040a05a7deb014abc145bab))
* filter expression node binding types ([#4340](https://github.com/rolldown/rolldown/issues/4340)) ([684e2b5](https://github.com/rolldown/rolldown/commit/684e2b5403d2c28a07ce9332e4d77eceb7f85ee5))
* **hmr:** handle import.meta.hot.invalidate ([#4339](https://github.com/rolldown/rolldown/issues/4339)) ([9725513](https://github.com/rolldown/rolldown/commit/9725513b9e223efe3b161b1bf99b6d602d33c00e))
* **hmr:** support sourcemap ([#4570](https://github.com/rolldown/rolldown/issues/4570)) ([d3b21fe](https://github.com/rolldown/rolldown/commit/d3b21fec55ad512378d237dd849fe49befa95925))
* implement filter expr filtering ([#4346](https://github.com/rolldown/rolldown/issues/4346)) ([d35f587](https://github.com/rolldown/rolldown/commit/d35f587365e0f23b7852f320cfe39a5af15fb0b7))
* introduce filter expression ([#4323](https://github.com/rolldown/rolldown/issues/4323)) ([21b9e45](https://github.com/rolldown/rolldown/commit/21b9e457d7e5096d4ec55559e98960a40737cce5))
* make `BindingMinifyOptions` fields optional ([#4307](https://github.com/rolldown/rolldown/issues/4307)) ([165d0b1](https://github.com/rolldown/rolldown/commit/165d0b1e0e77efe1e8bcda63cb80c890f85aa889)), closes [#4306](https://github.com/rolldown/rolldown/issues/4306)
* make RolldownBuild.watchFiles to async ([#4520](https://github.com/rolldown/rolldown/issues/4520)) ([4666852](https://github.com/rolldown/rolldown/commit/466685235be8802ba2cf91fc5576b5e00b466069))
* merge `emitDecoratorMetadata` from tsconfig.json ([#4555](https://github.com/rolldown/rolldown/issues/4555)) ([3f14835](https://github.com/rolldown/rolldown/commit/3f14835a80b8e0621bbb8386ade8772eff37f0ca)), closes [#4552](https://github.com/rolldown/rolldown/issues/4552)
* move filter related utils to `@rolldown/pluginutils` ([#4429](https://github.com/rolldown/rolldown/issues/4429)) ([f932e9e](https://github.com/rolldown/rolldown/commit/f932e9e5c33e6d132f44c972c6fb18e08ebc2d3d))
* output.virtualDirname ([#4521](https://github.com/rolldown/rolldown/issues/4521)) ([2e52dbc](https://github.com/rolldown/rolldown/commit/2e52dbc53e0fce99e0eeac56c1994afa59b31435))
* parse filter expr ([#4338](https://github.com/rolldown/rolldown/issues/4338)) ([e394f40](https://github.com/rolldown/rolldown/commit/e394f405d72eee41b292d062fb499c81af7e3bdb))
* partial preserveModules impl ([#4456](https://github.com/rolldown/rolldown/issues/4456)) ([a3b1473](https://github.com/rolldown/rolldown/commit/a3b147370cc0547f64101a892d63cd4b1a583216))
* **plugin/transform:** allow readonly array for options ([#4266](https://github.com/rolldown/rolldown/issues/4266)) ([86482ea](https://github.com/rolldown/rolldown/commit/86482eadb22fda599e8ae43141fde9b116380443))
* **plugin/vite-resolve:** port changes after Vite 6 ([#4269](https://github.com/rolldown/rolldown/issues/4269)) ([7c1fae3](https://github.com/rolldown/rolldown/commit/7c1fae383c48bed18ea1f67a6423be3327311c63)), closes [#4270](https://github.com/rolldown/rolldown/issues/4270)
* **plugin:** support code filter for `renderChunk` hook ([#4351](https://github.com/rolldown/rolldown/issues/4351)) ([ef1f137](https://github.com/rolldown/rolldown/commit/ef1f13714dfc5bfccf80827650e94ed4879d7108)), closes [#4231](https://github.com/rolldown/rolldown/issues/4231)
* **pluginutils:** `Query` filter impl ([#4535](https://github.com/rolldown/rolldown/issues/4535)) ([6c06a1c](https://github.com/rolldown/rolldown/commit/6c06a1c1b4192675f275b0182b8358e09b26c50f))
* **pluginutils:** add `exactRegex` and `prefixRegex` ([#4458](https://github.com/rolldown/rolldown/issues/4458)) ([7c58822](https://github.com/rolldown/rolldown/commit/7c588222f05d8fdacbc8247f6621c5a9b54cfcbb))
* **pluginutils:** add `makeIdFiltersToMatchWithQuery` function ([#4469](https://github.com/rolldown/rolldown/issues/4469)) ([75757fa](https://github.com/rolldown/rolldown/commit/75757fa25b795308ffb105287327be1ce97dfbd9))
* preserveModules ([#4457](https://github.com/rolldown/rolldown/issues/4457)) ([4b0464e](https://github.com/rolldown/rolldown/commit/4b0464e1ab162c2083c66e3ab610b7da9666373b))
* **rolldown_error:** add necessary `id` for some errors ([#4533](https://github.com/rolldown/rolldown/issues/4533)) ([d4801c4](https://github.com/rolldown/rolldown/commit/d4801c442bd94625e402866e82ca5742dc45017e)), closes [#4427](https://github.com/rolldown/rolldown/issues/4427)
* **rolldown_plugin_asset_import_meta_url:** initialize `assetImportMetaUrlPlugin` ([#4563](https://github.com/rolldown/rolldown/issues/4563)) ([84c4bac](https://github.com/rolldown/rolldown/commit/84c4bacb03bd872c498661b2f7149722f98270d2))
* **rolldown_plugin_asset:** align `resolveId` with `rolldown-vite` ([#4545](https://github.com/rolldown/rolldown/issues/4545)) ([d6e2358](https://github.com/rolldown/rolldown/commit/d6e23587d3e33c18b9994d09e3bfc23e1196eb3f))
* **rolldown_plugin_asset:** initialize asset plugin ([#4541](https://github.com/rolldown/rolldown/issues/4541)) ([51bbd05](https://github.com/rolldown/rolldown/commit/51bbd05ebab2e9eade6bb63bac149f1b9e8039d0))
* **rolldown_plugin_asset:** partially align `load` hook with `rolldown-vite` ([#4561](https://github.com/rolldown/rolldown/issues/4561)) ([a58cdd9](https://github.com/rolldown/rolldown/commit/a58cdd90edaa0c9ba90eed1db0d0400c8b695b1e))
* **rolldown_plugin_dynamic_import_vars:** basic support for custom resolver ([#4333](https://github.com/rolldown/rolldown/issues/4333)) ([ad5315f](https://github.com/rolldown/rolldown/commit/ad5315f21185ef7c1329b55a3ba0ed0b269ffbe1)), closes [#3968](https://github.com/rolldown/rolldown/issues/3968)
* **rolldown_plugin_dynamic_import_vars:** complete basic alignment work ([#4334](https://github.com/rolldown/rolldown/issues/4334)) ([1fd551c](https://github.com/rolldown/rolldown/commit/1fd551cdfdf0aa03069dba0b4f80d2d96bf4c54e)), closes [#3968](https://github.com/rolldown/rolldown/issues/3968)
* **rolldown_plugin_dynamic_import_vars:** introduce `resolver` option ([#4309](https://github.com/rolldown/rolldown/issues/4309)) ([89aa613](https://github.com/rolldown/rolldown/commit/89aa6134cefa8c9825b397d14d05ebaccf0779fe)), closes [#3968](https://github.com/rolldown/rolldown/issues/3968)
* **rolldown_plugin_dynamic_import_vars:** prepare for custom resolver ([#4328](https://github.com/rolldown/rolldown/issues/4328)) ([a15677b](https://github.com/rolldown/rolldown/commit/a15677be780012cf3e04f4fb15385d6e1ef225f5)), closes [#3968](https://github.com/rolldown/rolldown/issues/3968)
* **rolldown_plugin_dynamic_import_vars:** support `include` and `exclude` ([#4288](https://github.com/rolldown/rolldown/issues/4288)) ([c4e6cad](https://github.com/rolldown/rolldown/commit/c4e6cad59043a6242ebe9b9e809b04cbd22446d0)), closes [#3968](https://github.com/rolldown/rolldown/issues/3968)
* **rolldown_plugin_import_glob:** support brace expansion ([#4121](https://github.com/rolldown/rolldown/issues/4121)) ([54afc8f](https://github.com/rolldown/rolldown/commit/54afc8f95f37aa95fb6b9e365d1327bd6aa68354)), closes [#3982](https://github.com/rolldown/rolldown/issues/3982)
* **rolldown_plugin_reporter:** align the logic in `renderChunk` hook ([#4464](https://github.com/rolldown/rolldown/issues/4464)) ([db88da2](https://github.com/rolldown/rolldown/commit/db88da218c3094e32d626f15325a14c9625aedd5)), closes [#3968](https://github.com/rolldown/rolldown/issues/3968)
* **rolldown_plugin_reporter:** basically align behavior with `rolldown-vite` ([#4475](https://github.com/rolldown/rolldown/issues/4475)) ([90102d7](https://github.com/rolldown/rolldown/commit/90102d7b7a74319d2838619f504d661fcb9fff73)), closes [#3968](https://github.com/rolldown/rolldown/issues/3968)
* **rolldown_plugin_reporter:** further aligned with `rolldown-vite` ([#4476](https://github.com/rolldown/rolldown/issues/4476)) ([3985436](https://github.com/rolldown/rolldown/commit/3985436b66aeffd2e06b6aa3ee666b5794dab1d9)), closes [#3968](https://github.com/rolldown/rolldown/issues/3968)
* **rolldown_plugin_reporter:** prepare for chunk reporting ([#4449](https://github.com/rolldown/rolldown/issues/4449)) ([e270f24](https://github.com/rolldown/rolldown/commit/e270f244a2f4235933c4f651cddf5c47e8d6f5a9)), closes [#3968](https://github.com/rolldown/rolldown/issues/3968)
* **rolldown_plugin_web_worker_post:** align with `rolldown-vite` ([#4576](https://github.com/rolldown/rolldown/issues/4576)) ([0d7e06c](https://github.com/rolldown/rolldown/commit/0d7e06c831dc940ffebabc4ac9542f934c67b9bb))
* **rolldown:** `oxc_resolver` v8 ([#4392](https://github.com/rolldown/rolldown/issues/4392)) ([0de485b](https://github.com/rolldown/rolldown/commit/0de485b2860b5716262c8b83d3e6ccb712105ae8))
* **rolldown:** bump `oxc_resolver` to v7.0 ([#4367](https://github.com/rolldown/rolldown/issues/4367)) ([63723d5](https://github.com/rolldown/rolldown/commit/63723d59ba9b000e6b07f1f82ab60ba4bc996d62))
* **rolldown:** expose `experimental` entry for browser ([#4446](https://github.com/rolldown/rolldown/issues/4446)) ([069b04b](https://github.com/rolldown/rolldown/commit/069b04bd0a5d29f380269c679b57784b8b448c64))
* **rolldown:** expose `ModuleTypes` type ([#4506](https://github.com/rolldown/rolldown/issues/4506)) ([513a17a](https://github.com/rolldown/rolldown/commit/513a17ae165b4ff12a3bebf169f159b3355651e9))
* **rolldown:** oxc v0.66.0 ([#4275](https://github.com/rolldown/rolldown/issues/4275)) ([35ee0e7](https://github.com/rolldown/rolldown/commit/35ee0e79ff81f83343c56bab75f1df462bf6f323))
* **rolldown:** oxc v0.68.1 ([#4405](https://github.com/rolldown/rolldown/issues/4405)) ([4d4df59](https://github.com/rolldown/rolldown/commit/4d4df59349a721c113be5f5e1837b39556ea5742))
* **rolldown:** oxc v0.69.0 ([#4478](https://github.com/rolldown/rolldown/issues/4478)) ([4940c31](https://github.com/rolldown/rolldown/commit/4940c310c118c6c986c2f27b5ecb03c758dbd2da))
* **rolldown:** oxc v0.76.0 ([#4345](https://github.com/rolldown/rolldown/issues/4345)) ([86ed7c3](https://github.com/rolldown/rolldown/commit/86ed7c3a283997a636bd950a21aa249f9028ca12))
* **rolldown:** oxc_resolver v9.0.0 ([#4462](https://github.com/rolldown/rolldown/issues/4462)) ([9ef061d](https://github.com/rolldown/rolldown/commit/9ef061d317ac9d7c9b9dd3e1662b5b684e15a709))
* **rolldown:** update to `oxc@0.70.0` to refine printing comments  ([#4562](https://github.com/rolldown/rolldown/issues/4562)) ([1c583fe](https://github.com/rolldown/rolldown/commit/1c583fe976bea726eda3ad3c4aca7910040779bc))
* **rust:** execute cjs as it is ([#4465](https://github.com/rolldown/rolldown/issues/4465)) ([5becdcf](https://github.com/rolldown/rolldown/commit/5becdcf6104435d60b53b3b6819f4cae8cdd0532))
* **rust:** impl `Query` filter ([#4542](https://github.com/rolldown/rolldown/issues/4542)) ([8a5f862](https://github.com/rolldown/rolldown/commit/8a5f8621aeb28bc11b0b1e76abb7ad0aa5d4c71c))
* **rust:** initialize `rolldown_plugin_vite_css` ([#4418](https://github.com/rolldown/rolldown/issues/4418)) ([f2531a6](https://github.com/rolldown/rolldown/commit/f2531a61f9df57a032aa5f8fadac2055d29a9de9))
* **rust:** support `LegalComments::Inline` ([#4444](https://github.com/rolldown/rolldown/issues/4444)) ([6921d6c](https://github.com/rolldown/rolldown/commit/6921d6c3d2e7384bfc06322c9bdedb4dfc8be393))
* support extra IdParams and cleanUrl ([#4479](https://github.com/rolldown/rolldown/issues/4479)) ([55a9209](https://github.com/rolldown/rolldown/commit/55a9209fe1426e0bc896d2d4dd7d925f93291fbb))
* support variadic args for `or` and `and` filter expr ([#4371](https://github.com/rolldown/rolldown/issues/4371)) ([3fe195c](https://github.com/rolldown/rolldown/commit/3fe195c555b056a103b5fbd4fb2931152f36eb63))
* **types:** allow rollup plugin to be assigned to `plugins` option ([#4568](https://github.com/rolldown/rolldown/issues/4568)) ([42e496f](https://github.com/rolldown/rolldown/commit/42e496f4677d3ef0cf709458cbd1595c7aeaf6db))
* **types:** expose more watcher related types ([#4383](https://github.com/rolldown/rolldown/issues/4383)) ([21263e5](https://github.com/rolldown/rolldown/commit/21263e57e8ae0051236714914df549dee8d186a6))
* validate hoistTransitiveImports ([#4262](https://github.com/rolldown/rolldown/issues/4262)) ([ac258d5](https://github.com/rolldown/rolldown/commit/ac258d5f166a02d18a03a320b9ac163368cbedb7))
* **watcher:** support result.close at bundle end event ([#4423](https://github.com/rolldown/rolldown/issues/4423)) ([f579291](https://github.com/rolldown/rolldown/commit/f579291a9c5232c91efe972c77663376684abd1a))
* **watch:** support result.close at error event ([#4424](https://github.com/rolldown/rolldown/issues/4424)) ([7bb3956](https://github.com/rolldown/rolldown/commit/7bb3956aa6c9c1c0c60930ac92d6cce59a902dea))

### Bug Fixes

* `asset` ModuleType not available in TypeScript package ([#4489](https://github.com/rolldown/rolldown/issues/4489)) ([8ac92a4](https://github.com/rolldown/rolldown/commit/8ac92a4ecfb9fbc328033f936d6aac98c22af0ff))
* add debug information for debug wasm binaries ([#4549](https://github.com/rolldown/rolldown/issues/4549)) ([cc66f4b](https://github.com/rolldown/rolldown/commit/cc66f4b7189dfb3a248608d02f5962edb09b11f8))
* align directive rendering with esbuild ([#4557](https://github.com/rolldown/rolldown/issues/4557)) ([709eb63](https://github.com/rolldown/rolldown/commit/709eb63cb2f9c4b18bf7e36133251104a44cc45d))
* align resolve extension order with esbuild ([#4277](https://github.com/rolldown/rolldown/issues/4277)) ([6565161](https://github.com/rolldown/rolldown/commit/65651618e7ebcbfe2ba6652ae81321e53a3b4a56))
* align sanitize_filename with rollup ([#4244](https://github.com/rolldown/rolldown/issues/4244)) ([d53650c](https://github.com/rolldown/rolldown/commit/d53650c52948c411f2aaa9df535e8774f8bfa661))
* align validator with the actual types for `output.polyfillRequire` / `output.minify.deadCodeElimination` ([#4294](https://github.com/rolldown/rolldown/issues/4294)) ([6e313e1](https://github.com/rolldown/rolldown/commit/6e313e112489e99ee40538e6dcf5b1803b766e5d))
* chunk level directives rendering ([#4551](https://github.com/rolldown/rolldown/issues/4551)) ([a16881d](https://github.com/rolldown/rolldown/commit/a16881dfa13f25c6725eaa28bb848a7c8484d0c0)), closes [#4548](https://github.com/rolldown/rolldown/issues/4548)
* **chunk_exports:** Prevent duplicate external module imports ([#4408](https://github.com/rolldown/rolldown/issues/4408)) ([f90a05f](https://github.com/rolldown/rolldown/commit/f90a05f016455a72cf661b6d434bc5e554744e58)), closes [#4406](https://github.com/rolldown/rolldown/issues/4406)
* chunk.imports should include external imports ([#4315](https://github.com/rolldown/rolldown/issues/4315)) ([62dee06](https://github.com/rolldown/rolldown/commit/62dee0606e8f06223a1b2936db227d3ff05f78ac))
* **ci:** fix incorrect tag name for lychee-action ([#4352](https://github.com/rolldown/rolldown/issues/4352)) ([2531185](https://github.com/rolldown/rolldown/commit/253118585049e23900dceb1e4f3a0c4ff5d49729))
* **cli:** `ROLLUP_WATCH` should be set when config is loaded ([#4293](https://github.com/rolldown/rolldown/issues/4293)) ([f845728](https://github.com/rolldown/rolldown/commit/f8457289555112835d4e4f396a44575a4e1c97bf)), closes [#3967](https://github.com/rolldown/rolldown/issues/3967)
* **cli:** invalid type used with `--target` ([#4407](https://github.com/rolldown/rolldown/issues/4407)) ([8cce9fc](https://github.com/rolldown/rolldown/commit/8cce9fcca07bad471b6a0eb618498abdf10c06e8)), closes [#4387](https://github.com/rolldown/rolldown/issues/4387)
* consolidate log related types ([#4355](https://github.com/rolldown/rolldown/issues/4355)) ([0174d14](https://github.com/rolldown/rolldown/commit/0174d147eb17d2331ba4ca39adff7bb4e973e784)), closes [#4330](https://github.com/rolldown/rolldown/issues/4330)
* **debug:** ensure emitting json object per line and correct types ([#4295](https://github.com/rolldown/rolldown/issues/4295)) ([1d1fa3d](https://github.com/rolldown/rolldown/commit/1d1fa3df442ab4a66bfe4b72c3a70ac14866a5fb))
* don't minify .d.ts related chunk ([#4240](https://github.com/rolldown/rolldown/issues/4240)) ([d984417](https://github.com/rolldown/rolldown/commit/d984417561ae0d3234c7ae77a8ccfe35d556a5bc))
* dynamic import with backquotes are not bundled  ([#4435](https://github.com/rolldown/rolldown/issues/4435)) ([76c6380](https://github.com/rolldown/rolldown/commit/76c63802b9503c8e2c3548fd92334089020b55e7))
* ensure executing of plain imported cjs, fixes [#4443](https://github.com/rolldown/rolldown/issues/4443) ([#4468](https://github.com/rolldown/rolldown/issues/4468)) ([df9b4ee](https://github.com/rolldown/rolldown/commit/df9b4ee3236dccc9b0fd292b2325ff92d094a928))
* **hmr:** accpet deps ([#4314](https://github.com/rolldown/rolldown/issues/4314)) ([6aca0ce](https://github.com/rolldown/rolldown/commit/6aca0ce10ee9add434ddcef5f4f4dbbedd1f826c))
* **hmr:** aovid using quote_stmt, it make sourcemap panic ([#4571](https://github.com/rolldown/rolldown/issues/4571)) ([7b949a0](https://github.com/rolldown/rolldown/commit/7b949a0a1f5427adef9a2be4528f4f33c2c48763))
* **hmr:** enable incremental_build ([#4298](https://github.com/rolldown/rolldown/issues/4298)) ([7209cf3](https://github.com/rolldown/rolldown/commit/7209cf3ae682860518dfaa1df1f31794eba794d4))
* **hmr:** export full reload info if reach to hmr root ([#4242](https://github.com/rolldown/rolldown/issues/4242)) ([9b57bd3](https://github.com/rolldown/rolldown/commit/9b57bd30d6fc7cd7fa475a5a849d9e90f4bd9959))
* **hmr:** hmr chunk execute dependencies before create import.meta.hot ([#4320](https://github.com/rolldown/rolldown/issues/4320)) ([2a06dfe](https://github.com/rolldown/rolldown/commit/2a06dfe488f0b1cb3db0fcf7e30b64faa116c2c1))
* **hmr:** make sure runtime module imported for each chunk ([#4438](https://github.com/rolldown/rolldown/issues/4438)) ([4d444b0](https://github.com/rolldown/rolldown/commit/4d444b05ca8e441f33c6d19f6f512a465ad96fd1))
* **hmr:** normalize hmr chunk sourcemap sources path ([#4572](https://github.com/rolldown/rolldown/issues/4572)) ([6691f45](https://github.com/rolldown/rolldown/commit/6691f458bee690f0efd5d90ea9779c92bd958a7c))
* **hmr:** preserve original `import.meta.hot` when no HMR context exists ([#4391](https://github.com/rolldown/rolldown/issues/4391)) ([bc78fa1](https://github.com/rolldown/rolldown/commit/bc78fa19037424f8fd746dc5df88f64b323edfba)), closes [#4390](https://github.com/rolldown/rolldown/issues/4390) [#4370](https://github.com/rolldown/rolldown/issues/4370)
* **hmr:** reexport named declaration ([#4319](https://github.com/rolldown/rolldown/issues/4319)) ([3858daa](https://github.com/rolldown/rolldown/commit/3858daa3cbe7983b89d45278e6066dcbf8df9759))
* **hmr:** register cjs module exports ([#4511](https://github.com/rolldown/rolldown/issues/4511)) ([66f4623](https://github.com/rolldown/rolldown/commit/66f46233a8aeddfed089156d0b379c2a60690e45))
* **hmr:** remove duplicated import statment ([#4556](https://github.com/rolldown/rolldown/issues/4556)) ([69aebee](https://github.com/rolldown/rolldown/commit/69aebeeaceadbbffd18b83648b5fc4e2f4e035a3))
* **hmr:** rewrite import default/namespace reference ([#4299](https://github.com/rolldown/rolldown/issues/4299)) ([a0733d0](https://github.com/rolldown/rolldown/commit/a0733d075e020c8ced0df2f94a0da4f5f1cf10aa))
* **hmr:** rewrite import.meta.hot ([#4370](https://github.com/rolldown/rolldown/issues/4370)) ([7b030ee](https://github.com/rolldown/rolldown/commit/7b030ee361fa5962810070ed36cc05a3fec1ff38))
* moduleSideEffects function should also apply to external module ([#4305](https://github.com/rolldown/rolldown/issues/4305)) ([77c8935](https://github.com/rolldown/rolldown/commit/77c8935bf7b436353c927be88d355efe76da913f))
* **node:** add `type` property to `RenderedChunk` ([#4553](https://github.com/rolldown/rolldown/issues/4553)) ([8ca4fde](https://github.com/rolldown/rolldown/commit/8ca4fde1a2a02ded375f837483b882a569da90ed))
* **node:** ensure comments option got passed to rust ([#4527](https://github.com/rolldown/rolldown/issues/4527)) ([35571e5](https://github.com/rolldown/rolldown/commit/35571e5130e43272703d788b9f15f3473df7c816)), closes [#4491](https://github.com/rolldown/rolldown/issues/4491)
* **plugin/vite-resolve:** align default externalize logic with Vite ([#4247](https://github.com/rolldown/rolldown/issues/4247)) ([a37e704](https://github.com/rolldown/rolldown/commit/a37e70462a30974dd32b946da94e24d6fef007d9))
* **plugin/vite-resolve:** external was serialized incorrectly ([#4270](https://github.com/rolldown/rolldown/issues/4270)) ([3a98131](https://github.com/rolldown/rolldown/commit/3a98131b297fff2d84a6042ba3328b50e0c3056e))
* preserveModules cjs interop ([#4529](https://github.com/rolldown/rolldown/issues/4529)) ([1e7b372](https://github.com/rolldown/rolldown/commit/1e7b3724a6e575007bf474b78ccf108d98294e71))
* **render_chunk_exports:** add missing semicolon to exported value assignment ([#4498](https://github.com/rolldown/rolldown/issues/4498)) ([8739f25](https://github.com/rolldown/rolldown/commit/8739f25c49e89acf9617fa3f82d29345c114804d)), closes [#4459](https://github.com/rolldown/rolldown/issues/4459)
* **rolldown_plugin_transform:** use `or` instead of `xor` for `transformOptions.lang` ([#4587](https://github.com/rolldown/rolldown/issues/4587)) ([ef90f2e](https://github.com/rolldown/rolldown/commit/ef90f2e0438a3513dad1fbf35dc576832e593d28))
* **rolldown_transform_plugin:** incorrect detection of file extension ([#4241](https://github.com/rolldown/rolldown/issues/4241)) ([eae9262](https://github.com/rolldown/rolldown/commit/eae926288fae44e32be5714a1f1232704e3b84d6)), closes [#3881](https://github.com/rolldown/rolldown/issues/3881)
* **rust:** ramdom segmentation fault ([#4473](https://github.com/rolldown/rolldown/issues/4473)) ([42fd8e2](https://github.com/rolldown/rolldown/commit/42fd8e2ad47a87e7001d32c81cbdbab35838bba0))
* should not convert module to cjs if there exists toplevel this but no `module`,`exports`  used ([#4514](https://github.com/rolldown/rolldown/issues/4514)) ([f606614](https://github.com/rolldown/rolldown/commit/f6066141ee7137041f9537696d3fca6ef7934af7))
* should not throw for circular cjs imports, fixes [#3529](https://github.com/rolldown/rolldown/issues/3529) ([#4467](https://github.com/rolldown/rolldown/issues/4467)) ([3240812](https://github.com/rolldown/rolldown/commit/324081298a4d4f6552fc09a9eb22c2fb33c863b7))
* should rewrite jsx_name when jsx: preserve ([#4516](https://github.com/rolldown/rolldown/issues/4516)) ([d95f99e](https://github.com/rolldown/rolldown/commit/d95f99edd4cee8382772f59b9309bd59d7c3551f))
* should try to resolve `.json` extension by default ([#4276](https://github.com/rolldown/rolldown/issues/4276)) ([e84206b](https://github.com/rolldown/rolldown/commit/e84206b05786d0ce5e19c2e64b18b434cbe37895)), closes [#4274](https://github.com/rolldown/rolldown/issues/4274)
* transform hook code filter does not work ([#4386](https://github.com/rolldown/rolldown/issues/4386)) ([a720367](https://github.com/rolldown/rolldown/commit/a720367fe66ac9f2d23d773e92dd62843f691f24)), closes [#4379](https://github.com/rolldown/rolldown/issues/4379)
* use correct `FileTemplate` when enable `preserve_modules` ([#4517](https://github.com/rolldown/rolldown/issues/4517)) ([a779c8c](https://github.com/rolldown/rolldown/commit/a779c8c6bad426c1cde714b51e389e7f1c6a05b0))
* using original error if normalize original plugin error has error ([#4263](https://github.com/rolldown/rolldown/issues/4263)) ([d378cad](https://github.com/rolldown/rolldown/commit/d378cad79c117bfa8a7fb4d8a7aaccc3727e242c))
* **watch-cli:** call result.close at bundle end or error ([#4426](https://github.com/rolldown/rolldown/issues/4426)) ([55efe08](https://github.com/rolldown/rolldown/commit/55efe080fcc1a19416bf1c3340b077ec0ebf285b))
* **watch:** allow call `result.close` multiply times ([#4425](https://github.com/rolldown/rolldown/issues/4425)) ([e19ea12](https://github.com/rolldown/rolldown/commit/e19ea12106e9263f92fe96bb323e209ad88e68ad))

### Performance Improvements

* avoid allocating unnecessary memory in runtime for module_types map ([#4301](https://github.com/rolldown/rolldown/issues/4301)) ([f675391](https://github.com/rolldown/rolldown/commit/f6753919c611dc681525cff2cb3e69732bd27862))
* enable Wasm 128bit SIMD Extension ([#4484](https://github.com/rolldown/rolldown/issues/4484)) ([9d84ea8](https://github.com/rolldown/rolldown/commit/9d84ea8f66a5d2e6bb9db4776693207097ee377d))
* reduce wasm release binary size ([#4547](https://github.com/rolldown/rolldown/issues/4547)) ([bf53a10](https://github.com/rolldown/rolldown/commit/bf53a100edf1780d5a5aa41f0bc0459c5696543e))
* **rename:** reduce some string allocations ([#4487](https://github.com/rolldown/rolldown/issues/4487)) ([c51bc5c](https://github.com/rolldown/rolldown/commit/c51bc5c1433a6cdbf9a009f749ff689f1460d3b2))
* **rolldown_plugin_alias:** avoid unnecessary `to_string` allocations ([#4268](https://github.com/rolldown/rolldown/issues/4268)) ([bf18d12](https://github.com/rolldown/rolldown/commit/bf18d12bd4cca0e40185138c0753cbd9d445b3f2)), closes [#3968](https://github.com/rolldown/rolldown/issues/3968)
* **rolldown_plugin_json:** change `register_hook_usage` to return `HookUsage::Transform` ([#4375](https://github.com/rolldown/rolldown/issues/4375)) ([baf6ca1](https://github.com/rolldown/rolldown/commit/baf6ca18763f6d1a2aebfbd4532ca75b72428991)), closes [#3968](https://github.com/rolldown/rolldown/issues/3968)
* **rolldown:** reduce size of `ModuleLoaderMsg` from 1472 to 24 bytes ([#4496](https://github.com/rolldown/rolldown/issues/4496)) ([71d87e5](https://github.com/rolldown/rolldown/commit/71d87e5a472da0016be4f5581cab505a038828c3))
* **rolldown:** remove `ArcStr` usages in `finalize_chunks` ([#4494](https://github.com/rolldown/rolldown/issues/4494)) ([4dcaa00](https://github.com/rolldown/rolldown/commit/4dcaa009dfacad091c8485ae012de4317f65b7d8))
* **rolldown:** speedup `extract_hash_placeholders` with memchr ([#4495](https://github.com/rolldown/rolldown/issues/4495)) ([38fd4b8](https://github.com/rolldown/rolldown/commit/38fd4b8831d62c763c37b84b604530411f03b6f3))
* **rust:** improve `PreProcessor[#visit](https://github.com/rolldown/rolldown/issues/visit)_export_named_declaration` ([#4419](https://github.com/rolldown/rolldown/issues/4419)) ([f7e95d8](https://github.com/rolldown/rolldown/commit/f7e95d8f612feb696534fef5d5c269dcc5bddf4c))

### Miscellaneous Chores

* don't expose `And` class in pluginutils ([#4537](https://github.com/rolldown/rolldown/issues/4537)) ([985af6d](https://github.com/rolldown/rolldown/commit/985af6d846c0c85a61b19dcdd61d07c880074f38))
* expose `withFilter` function via `rolldown/filter` instead of `rolldown` ([#4369](https://github.com/rolldown/rolldown/issues/4369)) ([ead9749](https://github.com/rolldown/rolldown/commit/ead9749e14f5f1a3c1119d331f076fbc28027674))
* use camel cases for `package.json[#exports](https://github.com/rolldown/rolldown/issues/exports)` field ([#4366](https://github.com/rolldown/rolldown/issues/4366)) ([2fa6d40](https://github.com/rolldown/rolldown/commit/2fa6d4067ba6edceeab4d3f841cff6ff690ea160))



# [1.0.0-beta.8](https://github.com/rolldown/rolldown/compare/v1.0.0-beta.7...v1.0.0-beta.8) (2025-04-22)


### Bug Fixes

* attach more comments related to export decl ([#4219](https://github.com/rolldown/rolldown/issues/4219)) ([a684277](https://github.com/rolldown/rolldown/commit/a6842777f7ce910fabd8d9d1b418bcb4c2eea221))
* avoid empty line at the beginning of emitted chunks caused by file without imports ([#4179](https://github.com/rolldown/rolldown/issues/4179)) ([a3b5f54](https://github.com/rolldown/rolldown/commit/a3b5f54581e5a1f93d47249e9f21a474a5f74869)), closes [#4150](https://github.com/rolldown/rolldown/issues/4150)
* avoid hang if load error ([#4056](https://github.com/rolldown/rolldown/issues/4056)) ([f262f1f](https://github.com/rolldown/rolldown/commit/f262f1f516a2199436bf60b6e67233f96acd2753))
* avoid load again if load success at concurrent load ([#4156](https://github.com/rolldown/rolldown/issues/4156)) ([821fc82](https://github.com/rolldown/rolldown/commit/821fc8288d7be142e81ee02a812e9ec80cedd548))
* **build:** handle empty dependencies in rolldown configuration ([#4108](https://github.com/rolldown/rolldown/issues/4108)) ([934d75d](https://github.com/rolldown/rolldown/commit/934d75d75c75a351251056afbc54c443c79a88bf))
* clear js side cache at bundle fininsh ([#4158](https://github.com/rolldown/rolldown/issues/4158)) ([10afd61](https://github.com/rolldown/rolldown/commit/10afd612d0a63ca295d4ef1a64a14503ddfa2b70))
* convert severity of missing_export diagnostic to warning when importee module is ts/tsx ([#4147](https://github.com/rolldown/rolldown/issues/4147)) ([57ae80d](https://github.com/rolldown/rolldown/commit/57ae80def5352406eaace6df5d652d7e67080a16))
* correct bundle size calculation ([#3994](https://github.com/rolldown/rolldown/issues/3994)) ([748d395](https://github.com/rolldown/rolldown/commit/748d395c27f8366e4a7eebbdd0819f50aec65971))
* correct the behavior when multiple transform filter option are specified ([#4059](https://github.com/rolldown/rolldown/issues/4059)) ([92f7cc6](https://github.com/rolldown/rolldown/commit/92f7cc65534909f84c3d4fe4fdd42d80692ad2ca))
* **debug:** prevent multiple writes on the same file ([#4117](https://github.com/rolldown/rolldown/issues/4117)) ([9131bdc](https://github.com/rolldown/rolldown/commit/9131bdcf84246e2b15919a5e424866bdf7c4bf8e))
* deconfilct chunk name after name generated ([#4191](https://github.com/rolldown/rolldown/issues/4191)) ([1534f4a](https://github.com/rolldown/rolldown/commit/1534f4a7f444109b25d7b9b02590b14689de1ae0))
* don't unwatch files when close watcher ([#4105](https://github.com/rolldown/rolldown/issues/4105)) ([729b6cb](https://github.com/rolldown/rolldown/commit/729b6cb5fe78b6bd3022c7c75f2290d0094e7539)), closes [#4084](https://github.com/rolldown/rolldown/issues/4084)
* filter should not stabilize args.specifier ([#3972](https://github.com/rolldown/rolldown/issues/3972)) ([85c4bdd](https://github.com/rolldown/rolldown/commit/85c4bdd714af5c9245fc0a9cd3ed7b901f34e2be))
* filter test case ([#3975](https://github.com/rolldown/rolldown/issues/3975)) ([eeb1005](https://github.com/rolldown/rolldown/commit/eeb10051b0f5ddd144cd3cea5268cc4f332a93c3))
* **hmr:** should generate correct commonjs wrapper ([#4131](https://github.com/rolldown/rolldown/issues/4131)) ([afae765](https://github.com/rolldown/rolldown/commit/afae76571b3730a8484341936c4e1d1b2b670579)), closes [#4129](https://github.com/rolldown/rolldown/issues/4129)
* **hmr:** should register cjs exports differing from esm ([#4132](https://github.com/rolldown/rolldown/issues/4132)) ([ca47ca2](https://github.com/rolldown/rolldown/commit/ca47ca2888b0722fbce21cc271d30a359bd392f5))
* **isolated_declaration:** skip external id ([#4026](https://github.com/rolldown/rolldown/issues/4026)) ([8b7c824](https://github.com/rolldown/rolldown/commit/8b7c8241e4d105fec4a1a51afed14613a206e1d7))
* make sure flush before write into stdio ([#4157](https://github.com/rolldown/rolldown/issues/4157)) ([85dcb2a](https://github.com/rolldown/rolldown/commit/85dcb2aef20533a21a9ce3a4c70188e8358ff061))
* named_function_expression_argument ([#4074](https://github.com/rolldown/rolldown/issues/4074)) ([e95a392](https://github.com/rolldown/rolldown/commit/e95a39254b7e9da696854035f69b999019fa19e1)), closes [#4045](https://github.com/rolldown/rolldown/issues/4045) [#4061](https://github.com/rolldown/rolldown/issues/4061)
* **node:** ensure `this.meta.watchMode` is `true` in watch mode ([#3969](https://github.com/rolldown/rolldown/issues/3969)) ([7ddcaea](https://github.com/rolldown/rolldown/commit/7ddcaea299a491541638ff52cfabe92e4018c3d5)), closes [#3960](https://github.com/rolldown/rolldown/issues/3960)
* **node:** fix webcontainer fallback for cjs rolldown + silence pkg.pr.new check error ([#3958](https://github.com/rolldown/rolldown/issues/3958)) ([465ee23](https://github.com/rolldown/rolldown/commit/465ee23269f227633941edc62afa41804c21c08e))
* **node:** resolveId id filter is optional ([#3991](https://github.com/rolldown/rolldown/issues/3991)) ([c7e4434](https://github.com/rolldown/rolldown/commit/c7e443479fec9df0f99e9040fe856cc7b6870e04))
* preserve leading comments of export default decl ([#4122](https://github.com/rolldown/rolldown/issues/4122)) ([e3ab62a](https://github.com/rolldown/rolldown/commit/e3ab62aaf91a0589b209516aa48bdc9055ef6493)), closes [#4006](https://github.com/rolldown/rolldown/issues/4006)
* **rolldown_plugin_transform:** incorrect detection of `source_type` ([#4214](https://github.com/rolldown/rolldown/issues/4214)) ([bfe3e9e](https://github.com/rolldown/rolldown/commit/bfe3e9e7ab9434f345d691a0fd41a60e6fa3e736)), closes [#4203](https://github.com/rolldown/rolldown/issues/4203)
* **rolldown_utils:** normalize ids before filtering ([#3955](https://github.com/rolldown/rolldown/issues/3955)) ([dc245f0](https://github.com/rolldown/rolldown/commit/dc245f0c83a9cb6692ea717cd27f949c6e53ea7f))
* runtime module should get executed first always ([#3978](https://github.com/rolldown/rolldown/issues/3978)) ([1c679ee](https://github.com/rolldown/rolldown/commit/1c679eef9fa0160254d7725496db0347176d673c)), closes [#3650](https://github.com/rolldown/rolldown/issues/3650)
* **rust:** `preserve-legal` should correspond to `Comments::PreserveLegal` ([#4137](https://github.com/rolldown/rolldown/issues/4137)) ([cef50bd](https://github.com/rolldown/rolldown/commit/cef50bd09bb7618c9c21ead93d0c93af9323fc12)), closes [#4118](https://github.com/rolldown/rolldown/issues/4118)
* **rust:** avoid panic when `assetFileNames` hash length exceeds `22` ([#4019](https://github.com/rolldown/rolldown/issues/4019)) ([86710df](https://github.com/rolldown/rolldown/commit/86710dfb7fcc278a94bda48c6c3562bf8118aa0f)), closes [#4016](https://github.com/rolldown/rolldown/issues/4016)
* **rust:** crashes with object containing `shorthand` to `NaN` ([#4197](https://github.com/rolldown/rolldown/issues/4197)) ([7d55cda](https://github.com/rolldown/rolldown/commit/7d55cdafb69e74fd84cfaa2f0b2c55d480d58dd0)), closes [#4196](https://github.com/rolldown/rolldown/issues/4196) [#4123](https://github.com/rolldown/rolldown/issues/4123)
* support nested plugin for withFilter ([#4106](https://github.com/rolldown/rolldown/issues/4106)) ([698bd4c](https://github.com/rolldown/rolldown/commit/698bd4c3c355388b3ca1821ccc262ac121327d36))
* typo in example code ([#4167](https://github.com/rolldown/rolldown/issues/4167)) ([31e3f1b](https://github.com/rolldown/rolldown/commit/31e3f1babb9308fafc42b17ddbd722f85d482ca8))
* validate hmr option ([#4067](https://github.com/rolldown/rolldown/issues/4067)) ([c1d0963](https://github.com/rolldown/rolldown/commit/c1d096385913793f6e37cdd3b37ebdb03d4c85f0))


### Features

* add `withFilter` util function ([#4087](https://github.com/rolldown/rolldown/issues/4087)) ([c89829d](https://github.com/rolldown/rolldown/commit/c89829d3204b03e7837e6d5e5acd43a94b4602fe))
* add hmr option binding ([#4053](https://github.com/rolldown/rolldown/issues/4053)) ([cb9a009](https://github.com/rolldown/rolldown/commit/cb9a0094ad31d4d96daab5c25c5a1c75855bbeed))
* add hmr runtime implement option ([#4115](https://github.com/rolldown/rolldown/issues/4115)) ([dcaa72b](https://github.com/rolldown/rolldown/commit/dcaa72b1e900805618a3dcdb562bec1abd339f07))
* add hook usage meta ([#4181](https://github.com/rolldown/rolldown/issues/4181)) ([95255cf](https://github.com/rolldown/rolldown/commit/95255cf5f7140af3a1e64be2369832076b94390e))
* **cli:** enable `using` syntax sugar ([#4202](https://github.com/rolldown/rolldown/issues/4202)) ([59e9a96](https://github.com/rolldown/rolldown/commit/59e9a96ff2a530bc2b49a64b394437f8706dfbcb))
* **cli:** support `--input` option ([#4201](https://github.com/rolldown/rolldown/issues/4201)) ([6f18af6](https://github.com/rolldown/rolldown/commit/6f18af678f735036b36236ccc2479afd13345cb8)), closes [#4175](https://github.com/rolldown/rolldown/issues/4175)
* **cli:** support `ROLLUP_WATCH` and `ROLLDOWN_WATCH` environment variables ([#3967](https://github.com/rolldown/rolldown/issues/3967)) ([bc1eba7](https://github.com/rolldown/rolldown/commit/bc1eba75e7046fda952ad3001e7b6c9a2f2b1200)), closes [#3961](https://github.com/rolldown/rolldown/issues/3961)
* **debug:** add `DebugTracer` for improved tracing management ([#4182](https://github.com/rolldown/rolldown/issues/4182)) ([b8a64bb](https://github.com/rolldown/rolldown/commit/b8a64bbd5888833e4a247de936ec7e919f4e89df))
* **debug:** add `plugin_index` hook-related events ([#4148](https://github.com/rolldown/rolldown/issues/4148)) ([6a35d00](https://github.com/rolldown/rolldown/commit/6a35d00394e2150aac9a18bc82e7406003bc3464))
* **debug:** add events about `resolve_id` hook ([#4180](https://github.com/rolldown/rolldown/issues/4180)) ([0735e9a](https://github.com/rolldown/rolldown/commit/0735e9ab69d8a02087fbc7b9bd395f326fe1504c))
* **debug:** add lifecycle-related events ([#4168](https://github.com/rolldown/rolldown/issues/4168)) ([d9c5457](https://github.com/rolldown/rolldown/commit/d9c54570aecff21b8af326cbdef01f9d523c3946))
* **debug:** clean up opened files when bundler get dropped ([#4185](https://github.com/rolldown/rolldown/issues/4185)) ([baf201a](https://github.com/rolldown/rolldown/commit/baf201afae746cba82bf80397565cf42d3122e72))
* **debug:** rename `buildId` to `SessionId` ([#4184](https://github.com/rolldown/rolldown/issues/4184)) ([f67f29f](https://github.com/rolldown/rolldown/commit/f67f29ff3321a8abc815923e961f6cfe496a43dc))
* **debug:** set up event-based tracking system for bundler actions ([#4094](https://github.com/rolldown/rolldown/issues/4094)) ([bdc799c](https://github.com/rolldown/rolldown/commit/bdc799cbe6d7ace3b37a9e644c6abb1bae50ae58)), closes [#4049](https://github.com/rolldown/rolldown/issues/4049) [#4049](https://github.com/rolldown/rolldown/issues/4049)
* **debug:** trace load hook actions ([#4103](https://github.com/rolldown/rolldown/issues/4103)) ([0e12b69](https://github.com/rolldown/rolldown/commit/0e12b69aa74fc2a9a35942ddf6925256f3e41317))
* **debug:** unify naming ([#4183](https://github.com/rolldown/rolldown/issues/4183)) ([211c9df](https://github.com/rolldown/rolldown/commit/211c9df576cd639f03e2802753f5cf8a4e6713a1))
* **docs:** generate `llms.txt` and `llms-full.txt` ([#3979](https://github.com/rolldown/rolldown/issues/3979)) ([ef1de29](https://github.com/rolldown/rolldown/commit/ef1de299a04abb3a3d26f96e84702b555d57627c))
* export `ResolveIdExtraOptions` ([#4050](https://github.com/rolldown/rolldown/issues/4050)) ([25f3c61](https://github.com/rolldown/rolldown/commit/25f3c6106c6fdad5558da0acb43acdfc542dc1e6))
* **hmr/poc:** support HMR on situation that adds modules or modifies import statements ([#3965](https://github.com/rolldown/rolldown/issues/3965)) ([7452fa0](https://github.com/rolldown/rolldown/commit/7452fa03edb3c4cda04baaac0b9f57a5ae047605))
* **hmr:** allow to configurate connected address of dev server ([#4001](https://github.com/rolldown/rolldown/issues/4001)) ([06fa09d](https://github.com/rolldown/rolldown/commit/06fa09d74ae4a80e23c4b6152838399a257cd2e6))
* **hmr:** export hmr boundary info ([#4116](https://github.com/rolldown/rolldown/issues/4116)) ([149a7b9](https://github.com/rolldown/rolldown/commit/149a7b9c93492ff2f4e6b284b7ccf4ecc0000d71))
* **hmr:** prepare hmr support on nodejs ([#4002](https://github.com/rolldown/rolldown/issues/4002)) ([d97c9cc](https://github.com/rolldown/rolldown/commit/d97c9cc5e63264162b4c0bf5f537110ab3647e70))
* initial implementation of `@rolldown/browser` ([#4112](https://github.com/rolldown/rolldown/issues/4112)) ([a0a7d95](https://github.com/rolldown/rolldown/commit/a0a7d9519e4dd93512c4ebb1e709a75a6b65efbc)), closes [#4012](https://github.com/rolldown/rolldown/issues/4012)
* **node:** auto fallback wasm binding on webcontainer ([#3922](https://github.com/rolldown/rolldown/issues/3922)) ([c6972f4](https://github.com/rolldown/rolldown/commit/c6972f453c76881b46d50be386a8d9b1c3d43492))
* **node:** expose `RenderedModule` type ([#3963](https://github.com/rolldown/rolldown/issues/3963)) ([311f70c](https://github.com/rolldown/rolldown/commit/311f70cc45177a067b924dce703b9d3db49ae318))
* optimize dep unchanged ([#3974](https://github.com/rolldown/rolldown/issues/3974)) ([c586f68](https://github.com/rolldown/rolldown/commit/c586f684d721bc21eada583ebc5fb638f7911727))
* partial builtin report plugin ([#4136](https://github.com/rolldown/rolldown/issues/4136)) ([6ad31d7](https://github.com/rolldown/rolldown/commit/6ad31d74a76041411de75dfa588f36ba92fc0df2))
* rolldown_filter_analyzer ([#4232](https://github.com/rolldown/rolldown/issues/4232)) ([d0ad1e9](https://github.com/rolldown/rolldown/commit/d0ad1e9299b9e444dd65512ff0a1c5f653626ed5))
* **rolldown_plugin_transform:** enable `env` and `assumptions` ([#4211](https://github.com/rolldown/rolldown/issues/4211)) ([73f1910](https://github.com/rolldown/rolldown/commit/73f19108c8b65f8306ddfd1772f15528dbd454bb)), closes [#3968](https://github.com/rolldown/rolldown/issues/3968)
* **rolldown_plugin_transform:** support `assumptions` option in `transform_options` ([#4209](https://github.com/rolldown/rolldown/issues/4209)) ([932b46f](https://github.com/rolldown/rolldown/commit/932b46f731cb91af734a6b32d0c72f7bdd7672be)), closes [#3968](https://github.com/rolldown/rolldown/issues/3968)
* **rolldown_plugin_transform:** support `target` option in `transform_options` ([#4210](https://github.com/rolldown/rolldown/issues/4210)) ([e00c4ca](https://github.com/rolldown/rolldown/commit/e00c4ca2ffeccf027bbf5b07e5e234dce6ba2a5c)), closes [#3968](https://github.com/rolldown/rolldown/issues/3968)
* **rolldown:** oxc v0.65.0 ([#4235](https://github.com/rolldown/rolldown/issues/4235)) ([d63b266](https://github.com/rolldown/rolldown/commit/d63b266fca55edeeeb9d38938ff1a4f824eb67fc)), closes [#4166](https://github.com/rolldown/rolldown/issues/4166) [#4198](https://github.com/rolldown/rolldown/issues/4198)
* support MinimalPluginContext for outputOptions hook ([#3993](https://github.com/rolldown/rolldown/issues/3993)) ([19eaa2d](https://github.com/rolldown/rolldown/commit/19eaa2d5802016bc6329db34b8623e140e02c7ea))
* support RolldownBuild#watchFiles ([#4057](https://github.com/rolldown/rolldown/issues/4057)) ([34efde9](https://github.com/rolldown/rolldown/commit/34efde9095521f4aa6ec54df1e668164b0c2ccea))


### Performance Improvements

* aovid unnecessary allocate at PreliminaryFilename new ([#4194](https://github.com/rolldown/rolldown/issues/4194)) ([c391b5e](https://github.com/rolldown/rolldown/commit/c391b5e613dccdd2ff297b77d05dafd602d974fe))
* avoid clone immutable scoping field ([#3980](https://github.com/rolldown/rolldown/issues/3980)) ([d024a79](https://github.com/rolldown/rolldown/commit/d024a79ef301d57277d138116487ce75d0f4b2b2))
* avoid lock contention ([#3984](https://github.com/rolldown/rolldown/issues/3984)) ([86fb332](https://github.com/rolldown/rolldown/commit/86fb3326fb8c845628cbd22e14c662248f807ebc))
* cache renderChunk meta ([#3999](https://github.com/rolldown/rolldown/issues/3999)) ([e117288](https://github.com/rolldown/rolldown/commit/e11728818f6c4fd389308f4b2c1d8503e5ce5af2))
* code filter contains ([#3973](https://github.com/rolldown/rolldown/issues/3973)) ([acbb339](https://github.com/rolldown/rolldown/commit/acbb3399de9ecbe2e471276cc8792e0128987558))
* lazy renderChunk meta#chunks ([#4008](https://github.com/rolldown/rolldown/issues/4008)) ([d5262e2](https://github.com/rolldown/rolldown/commit/d5262e21ebafb1ef6a4992b50142bd872dccdce5))
* make js plugin tracing more accurate ([#4204](https://github.com/rolldown/rolldown/issues/4204)) ([76ba8e5](https://github.com/rolldown/rolldown/commit/76ba8e5e9dea8133ddd7af0980711d5fd2f530cc))
* **node:** cache properties of `OutputChunk` and `OutputAsset` ([#4063](https://github.com/rolldown/rolldown/issues/4063)) ([f32a14f](https://github.com/rolldown/rolldown/commit/f32a14fdc5efd8eb4e6aed45ce2ff1cd9780655e))
* **rolldown_plugin_json:** use `concat_string!` instead of `format!` ([#4030](https://github.com/rolldown/rolldown/issues/4030)) ([825c877](https://github.com/rolldown/rolldown/commit/825c877b56a601701d09104521f25af3a5b938bd))
* **rust:** use `array` instead of `phf_set` ([#4079](https://github.com/rolldown/rolldown/issues/4079)) ([d30dcc5](https://github.com/rolldown/rolldown/commit/d30dcc575ec323b9c49e91d036ec16f14085b5b3))
* **rust:** use `into_owned` instead of `to_string` to avoid unnecessary allocation ([#4149](https://github.com/rolldown/rolldown/issues/4149)) ([5b4032a](https://github.com/rolldown/rolldown/commit/5b4032ac460644a9f9ae3ed7ad5e51420c5f5565))
* **rust:** use `phf_set` for large datasets ([#4216](https://github.com/rolldown/rolldown/issues/4216)) ([c2596d3](https://github.com/rolldown/rolldown/commit/c2596d3b89e35e161104600ff745f2ee73c504e4)), closes [#4079](https://github.com/rolldown/rolldown/issues/4079)
* skip empty hook with `hook_usage` meta ([#4187](https://github.com/rolldown/rolldown/issues/4187)) ([37e71eb](https://github.com/rolldown/rolldown/commit/37e71eb010648a8828c10e70c7300b27c979a2f8))
* skip empty js hook for js plugin ([#4192](https://github.com/rolldown/rolldown/issues/4192)) ([5e53564](https://github.com/rolldown/rolldown/commit/5e53564babb8b931338d456d98ed72ddcc4e72e7))



# [1.0.0-beta.7](https://github.com/rolldown/rolldown/compare/v1.0.0-beta.6...v1.0.0-beta.7) (2025-03-24)


### Bug Fixes

* check dummy record in all import_records.iter ([#3930](https://github.com/rolldown/rolldown/issues/3930)) ([2519603](https://github.com/rolldown/rolldown/commit/25196034858e6be8ec769be230ead12474da89c5))
* **ci:** fix broken metric workflow ([#3944](https://github.com/rolldown/rolldown/issues/3944)) ([1df6407](https://github.com/rolldown/rolldown/commit/1df64079216086d4bd96ba05bb91a553702a173c))
* determine a module side effects for `require` a module that can't analyze statically ([#3928](https://github.com/rolldown/rolldown/issues/3928)) ([3909fcb](https://github.com/rolldown/rolldown/commit/3909fcb2f9c23a6b84ce59e4ae93801ca6801ea4)), closes [#3926](https://github.com/rolldown/rolldown/issues/3926)
* mark export all stmt as side effects free ([#3897](https://github.com/rolldown/rolldown/issues/3897)) ([0f36c01](https://github.com/rolldown/rolldown/commit/0f36c017606efc6736a893cd5b3398a35a6c5066))
* metrix ci broken ([#3924](https://github.com/rolldown/rolldown/issues/3924)) ([f115e7a](https://github.com/rolldown/rolldown/commit/f115e7a34b264fe83930d6127f833053bbb4ab83))
* normalize oxc minify keep_names with keep_names option ([#3948](https://github.com/rolldown/rolldown/issues/3948)) ([126f3e0](https://github.com/rolldown/rolldown/commit/126f3e08ec67c6a6025537fac411d67628e06b2b))
* reuse `JSON.parse` wrapper from `oxc-parser` ([#3904](https://github.com/rolldown/rolldown/issues/3904)) ([1726b6a](https://github.com/rolldown/rolldown/commit/1726b6ac303a0d0efc9b70822d38c5417df09764))
* **tasks/generator:** ensure `just update-generated-code` runs correctly on `Windows` ([#3900](https://github.com/rolldown/rolldown/issues/3900)) ([ca73e82](https://github.com/rolldown/rolldown/commit/ca73e82817a536ea3c49e7f3641f52405e8b459c))
* **tasks/generator:** ensure consistent path on `Windows` ([#3901](https://github.com/rolldown/rolldown/issues/3901)) ([df042b0](https://github.com/rolldown/rolldown/commit/df042b0b4034291e53763c3ee736479cfe6eb8ef))


### Features

* add data structure that used for incremental build ([#3932](https://github.com/rolldown/rolldown/issues/3932)) ([8c0b950](https://github.com/rolldown/rolldown/commit/8c0b95028333d0aade29592fc365f2163156e7d0))
* add HookTransformAstArgs#stable_id ([#3920](https://github.com/rolldown/rolldown/issues/3920)) ([15cb25a](https://github.com/rolldown/rolldown/commit/15cb25af40d189531d3456ab70ffee0e3b7f82de))
* add isolated_declaration plugin ([#3894](https://github.com/rolldown/rolldown/issues/3894)) ([529a7be](https://github.com/rolldown/rolldown/commit/529a7bee5585e37f5aabc541b23e3384913f91bc))
* **hmr/poc:** change visited map to use `VisitState` enum for better state management ([#3908](https://github.com/rolldown/rolldown/issues/3908)) ([1fd3698](https://github.com/rolldown/rolldown/commit/1fd3698233f4b2fe960fb9aa1797604cda234471))
* incremental rebuild ([#3934](https://github.com/rolldown/rolldown/issues/3934)) ([0b03e35](https://github.com/rolldown/rolldown/commit/0b03e352b1795132ec895bf369280e2648fd173b))
* **isolated_declaration:** add `stripInternal` option ([#3902](https://github.com/rolldown/rolldown/issues/3902)) ([8f5f8f1](https://github.com/rolldown/rolldown/commit/8f5f8f11eb06c497eb50d907d7fd5258ad8fe4de))
* **isolated-declaration:** emit typing for imported or exported typing module ([#3910](https://github.com/rolldown/rolldown/issues/3910)) ([458a62d](https://github.com/rolldown/rolldown/commit/458a62d78fef46776ca2e8b8990d41a31d69d998))
* make ScanStageOutput cloneable ([#3923](https://github.com/rolldown/rolldown/issues/3923)) ([47fe675](https://github.com/rolldown/rolldown/commit/47fe67560573aa70544b27f8c763f5b8b96a8284))
* make transform_ast hook to async ([#3891](https://github.com/rolldown/rolldown/issues/3891)) ([b150e1b](https://github.com/rolldown/rolldown/commit/b150e1b75b467e50836b89251056a92720357bea))
* **node/hmr:** ensure only invoke hmr process if there are live connections ([#3907](https://github.com/rolldown/rolldown/issues/3907)) ([9cb523f](https://github.com/rolldown/rolldown/commit/9cb523f6195a68e822c7cf41ae7d0fa585e933d7))
* **node/wasi:** add private `@rolldown/wasi` to prepare to distribute wasm binary in a friendly way ([#3925](https://github.com/rolldown/rolldown/issues/3925)) ([53faf0c](https://github.com/rolldown/rolldown/commit/53faf0c5d4c398d7d9f6c1bddf279ac5c4c071bc))
* remove unused import(...) if importee doesn't have side-effects ([#3911](https://github.com/rolldown/rolldown/issues/3911)) ([0ac283b](https://github.com/rolldown/rolldown/commit/0ac283bfc0f408e4c4a2ae0579d64b1a397e4e5b)), closes [#2827](https://github.com/rolldown/rolldown/issues/2827)
* support meta#chunks at render_chunk hook ([#3898](https://github.com/rolldown/rolldown/issues/3898)) ([8fd9481](https://github.com/rolldown/rolldown/commit/8fd94815d2714d90efdcc0c7f4100ed5007f14ae))


### Performance Improvements

* aovid RollupRenderedChunk clone ([#3909](https://github.com/rolldown/rolldown/issues/3909)) ([82749d8](https://github.com/rolldown/rolldown/commit/82749d8c2e7651b662912de0eef985d58cef9731))
* avoid chunk content clone ([#3916](https://github.com/rolldown/rolldown/issues/3916)) ([c33ea77](https://github.com/rolldown/rolldown/commit/c33ea772dc6c286c53a72ccf18b36cad3866322f))
* **rolldown_binding:** upgrade napi-build ([#3927](https://github.com/rolldown/rolldown/issues/3927)) ([5da7efb](https://github.com/rolldown/rolldown/commit/5da7efb1c26441859803337f06aa659537588e9a))
* use `Vec::with_capacity` ([#3938](https://github.com/rolldown/rolldown/issues/3938)) ([113eb98](https://github.com/rolldown/rolldown/commit/113eb985ee6aeccc5a3af4cf31486fc96d11b49e))
* use multi threaded Runtime on wasi target ([#3876](https://github.com/rolldown/rolldown/issues/3876)) ([fb9cbde](https://github.com/rolldown/rolldown/commit/fb9cbdeeb650e1a47b38e02f9be48e4e6ce8ae4a))



# [1.0.0-beta.6](https://github.com/rolldown/rolldown/compare/v1.0.0-beta.5...v1.0.0-beta.6) (2025-03-17)



# [1.0.0-beta.5](https://github.com/rolldown/rolldown/compare/v1.0.0-beta.4...v1.0.0-beta.5) (2025-03-17)


### Bug Fixes

* **cli:** fix and improve error message when validating cli options ([#3813](https://github.com/rolldown/rolldown/issues/3813)) ([64993bb](https://github.com/rolldown/rolldown/commit/64993bb64738f530539228201df1f3e98d2ca562))
* **cli:** remove duplicate error message in stack trace ([#3828](https://github.com/rolldown/rolldown/issues/3828)) ([79fea00](https://github.com/rolldown/rolldown/commit/79fea00530f310ada3a880c8bb0840476d4d9848))
* dataurl plugin resolve panic ([#3822](https://github.com/rolldown/rolldown/issues/3822)) ([251e281](https://github.com/rolldown/rolldown/commit/251e281cb707277a9eb4d51d8bdcce757a2c8fc6))
* generate sourcemapMappingUrl reference properly ([#3849](https://github.com/rolldown/rolldown/issues/3849)) ([2b0f20f](https://github.com/rolldown/rolldown/commit/2b0f20f6a0c2bcc7350b0a22c10df7d0d4c528e4)), closes [#3845](https://github.com/rolldown/rolldown/issues/3845)
* **renovate:** dependencyDashboard is not a preset ([#3862](https://github.com/rolldown/rolldown/issues/3862)) ([607e112](https://github.com/rolldown/rolldown/commit/607e1120df1b07736525f1a87470d7f81906ac7a))


### Features

* calculate relative external module rendered path ([#3855](https://github.com/rolldown/rolldown/issues/3855)) ([e0cd410](https://github.com/rolldown/rolldown/commit/e0cd410a91e78bb36aafa1ac26fc4d56cc02bcb5))
* emit diagnostic when configuration field conflict ([#3854](https://github.com/rolldown/rolldown/issues/3854)) ([d0e0a63](https://github.com/rolldown/rolldown/commit/d0e0a6386d761fe3010606bd542293c46f3e4715))
* filter out diagnostics disabled in checks options ([#3846](https://github.com/rolldown/rolldown/issues/3846)) ([4524a58](https://github.com/rolldown/rolldown/commit/4524a58057954b21535f21877a31386b7c83f39a))
* **hmr/rust:** invoke `transform` hook on affected hmr module ([#3866](https://github.com/rolldown/rolldown/issues/3866)) ([296d605](https://github.com/rolldown/rolldown/commit/296d605c2fc3a037abf8cb2e81d735810704cdf8))
* **hmr/rust:** reuse existing `ModuleLoader` ([#3865](https://github.com/rolldown/rolldown/issues/3865)) ([b4dd209](https://github.com/rolldown/rolldown/commit/b4dd2094ca211f844915dd5cf90a3c4fec87c448))
* **hmr:** enhance error handling by wrapping program body in a try-catch block ([#3825](https://github.com/rolldown/rolldown/issues/3825)) ([007805a](https://github.com/rolldown/rolldown/commit/007805ae6942ea96578ff88bedb59f89f29e0c24))
* **hmr:** inject `ModuleHotContext` for each module ([#3832](https://github.com/rolldown/rolldown/issues/3832)) ([30b9a0b](https://github.com/rolldown/rolldown/commit/30b9a0bf53b7fde22a94c43486880e9d1b1e933c))
* **hmr:** support HMR on editing non-hmr-boundary module ([#3853](https://github.com/rolldown/rolldown/issues/3853)) ([568197a](https://github.com/rolldown/rolldown/commit/568197a06444809bf44642d88509313ee2735594))
* **hmr:** wrap module code into a function ([#3851](https://github.com/rolldown/rolldown/issues/3851)) ([8a7b7d1](https://github.com/rolldown/rolldown/commit/8a7b7d11484c1f3d2d604ed8e1ff7ba4752ab26c))
* memorize importer's idx for each moodule ([#3852](https://github.com/rolldown/rolldown/issues/3852)) ([3a7758f](https://github.com/rolldown/rolldown/commit/3a7758f043c0054f344ef781602eea965b421c29))
* support absoulte/relative external ([#3834](https://github.com/rolldown/rolldown/issues/3834)) ([d8f0bcb](https://github.com/rolldown/rolldown/commit/d8f0bcbe3bde51cc49fbada75f32560c43f51f95))


### Performance Improvements

* use mimalloc-safe which is maintaining by us ([#3869](https://github.com/rolldown/rolldown/issues/3869)) ([6b9d148](https://github.com/rolldown/rolldown/commit/6b9d148d7b0d5b182cdba42fbc5bafeb80d7c9af))



# [1.0.0-beta.4](https://github.com/rolldown/rolldown/compare/v1.0.0-beta.3...v1.0.0-beta.4) (2025-03-11)


### Bug Fixes

*  imports with only types are removed even if verbatimModuleSyntax is true ([#3784](https://github.com/rolldown/rolldown/issues/3784)) ([ff8d19d](https://github.com/rolldown/rolldown/commit/ff8d19d25a26f115bcaf7dc3243eb98d7894c057)), closes [#3777](https://github.com/rolldown/rolldown/issues/3777)
*  windows panic ([#3436](https://github.com/rolldown/rolldown/issues/3436)) ([bc5b1e7](https://github.com/rolldown/rolldown/commit/bc5b1e73d48107643150d5492cd3b82c4484c0b9))
* `keepNames` with function declaration panic ([#3687](https://github.com/rolldown/rolldown/issues/3687)) ([6016b3c](https://github.com/rolldown/rolldown/commit/6016b3cfd91eba5fe0945ed28927b706a0f0d959))
* add `sequential?: boolean` type for rollup compat ([#3442](https://github.com/rolldown/rolldown/issues/3442)) ([e9daf93](https://github.com/rolldown/rolldown/commit/e9daf9390ddb692f2d4e672eebb83b9a0dc2385d))
* add wrong is_cjs_reexport flag ([#3532](https://github.com/rolldown/rolldown/issues/3532)) ([c2bcb3d](https://github.com/rolldown/rolldown/commit/c2bcb3d232db048314649959a0ce3c09b8db7d7e))
* **advanced_chunks:** unexpected behavior with `maxSize` ([#3641](https://github.com/rolldown/rolldown/issues/3641)) ([8b59091](https://github.com/rolldown/rolldown/commit/8b59091161bbcb4626ab41c8e0e22e656f009b5c))
* avoid generate asset filename panic if emit file with fileName ([#3423](https://github.com/rolldown/rolldown/issues/3423)) ([794a63d](https://github.com/rolldown/rolldown/commit/794a63d0459f9e5e2b5450647d70f9b06ecb9a0c))
* avoid preliminary filenames conflict ([#3460](https://github.com/rolldown/rolldown/issues/3460)) ([aeb352c](https://github.com/rolldown/rolldown/commit/aeb352cd1120781a787625f570c36d222a4ef810))
* auto polyfill import.meta.* in none esm format ([#3454](https://github.com/rolldown/rolldown/issues/3454)) ([287a616](https://github.com/rolldown/rolldown/commit/287a616883fd028f5000942b7aaeff4126f4d3d4))
* avoid call outputOptions hook at close ([#3601](https://github.com/rolldown/rolldown/issues/3601)) ([48b1b86](https://github.com/rolldown/rolldown/commit/48b1b863e9fddeccbdbbd98641bb28597506e8f5))
* avoid duplicated `init_esm` call ([#3707](https://github.com/rolldown/rolldown/issues/3707)) ([6eafc2f](https://github.com/rolldown/rolldown/commit/6eafc2f5916625c5d1b5267a4aca55b5ba1b2c1f))
* build-binding script should reuse passed args ([#3774](https://github.com/rolldown/rolldown/issues/3774)) ([77623da](https://github.com/rolldown/rolldown/commit/77623da18bc9fe63baa6a8b2155afa28c60fdb8e))
* chunk sorting total order ([#3451](https://github.com/rolldown/rolldown/issues/3451)) ([73116ab](https://github.com/rolldown/rolldown/commit/73116ab642701be42eda64761074e383a9ae801a))
* chunk.modules should order by exec_order ([#3638](https://github.com/rolldown/rolldown/issues/3638)) ([8219a83](https://github.com/rolldown/rolldown/commit/8219a835bd92d5f7eaa924cd01f8e6719a828ca5))
* **cjs:** ensure esm namespace always exist ([#3693](https://github.com/rolldown/rolldown/issues/3693)) ([f0301a6](https://github.com/rolldown/rolldown/commit/f0301a65a4a80ea8a43029776627278a77e693be)), closes [#3619](https://github.com/rolldown/rolldown/issues/3619) [#3529](https://github.com/rolldown/rolldown/issues/3529)
* cli doesn't show config loading error details ([#3464](https://github.com/rolldown/rolldown/issues/3464)) ([e048f55](https://github.com/rolldown/rolldown/commit/e048f557aa4b84f870b58fce76b28c7b0810253e))
* **cli:** cli default option ([#3613](https://github.com/rolldown/rolldown/issues/3613)) ([93d8731](https://github.com/rolldown/rolldown/commit/93d8731804f253b1ac16c8eb8541148448f072cf))
* consider require `ExpressionIdentfier` as a import record ([#3428](https://github.com/rolldown/rolldown/issues/3428)) ([2410ee7](https://github.com/rolldown/rolldown/commit/2410ee7927cacdc687ea3089ba6bade26f574bdd)), closes [#3395](https://github.com/rolldown/rolldown/issues/3395)
* deconflict export alias ([#3457](https://github.com/rolldown/rolldown/issues/3457)) ([fdb55d5](https://github.com/rolldown/rolldown/commit/fdb55d537d1502c9fab0c91365d6c802cc71214b))
* diagnostic in minified file significantly slow down the build ([#3498](https://github.com/rolldown/rolldown/issues/3498)) ([2efa799](https://github.com/rolldown/rolldown/commit/2efa7998f2a47319c15e4dfaed2e21527bed8e5d)), closes [#3455](https://github.com/rolldown/rolldown/issues/3455)
* don't enable drop_console when compress is true ([#3639](https://github.com/rolldown/rolldown/issues/3639)) ([2382047](https://github.com/rolldown/rolldown/commit/2382047bfa55c4eb7aaed466c34a1212a0c71a23))
* entry chunk name should respect sanitizeFileName ([#3618](https://github.com/rolldown/rolldown/issues/3618)) ([4ec8869](https://github.com/rolldown/rolldown/commit/4ec88694630df38f97892604199d9d6289299cd5))
* escape import/export module name and json key ([#3458](https://github.com/rolldown/rolldown/issues/3458)) ([4ccaf24](https://github.com/rolldown/rolldown/commit/4ccaf24ced490c6ad9c89e3e2d9c2fbe40d5e420))
* fix `ROLLUP_FILE_URL` for emitted chunks ([#3507](https://github.com/rolldown/rolldown/issues/3507)) ([63b4e88](https://github.com/rolldown/rolldown/commit/63b4e88582f041b7d5ed79964f03928f148600d1))
* glob import from parent dir ([#3614](https://github.com/rolldown/rolldown/issues/3614)) ([59c5da5](https://github.com/rolldown/rolldown/commit/59c5da53e6f459f929a7040ae7f160f76ffacd33))
* **glob_import:** return absolute paths for absolute globs ([#3701](https://github.com/rolldown/rolldown/issues/3701)) ([eec83c9](https://github.com/rolldown/rolldown/commit/eec83c9bd9f203d9db91afc8272236bc105419de))
* **hash:** should calculate hash dependenies correctly ([#3522](https://github.com/rolldown/rolldown/issues/3522)) ([c34d2d4](https://github.com/rolldown/rolldown/commit/c34d2d463e664cd7aee132fd78db887a3a496f58))
* **hash:** should compute cross chunk dependecis in stable order ([#3524](https://github.com/rolldown/rolldown/issues/3524)) ([52119c7](https://github.com/rolldown/rolldown/commit/52119c74ee42c59ef0913ff22eff7d100823cdc8)), closes [#3453](https://github.com/rolldown/rolldown/issues/3453)
* load mts config without type module ([#3750](https://github.com/rolldown/rolldown/issues/3750)) ([64998fb](https://github.com/rolldown/rolldown/commit/64998fbb3ac66bb6cfd1dd9ea4420a0c2751e52a))
* make default sanitizeFilename compatible with rollup ([#3431](https://github.com/rolldown/rolldown/issues/3431)) ([67ec7d7](https://github.com/rolldown/rolldown/commit/67ec7d72901c1e9963d0e498f38033b428d07e19)), closes [#3418](https://github.com/rolldown/rolldown/issues/3418) [#3418](https://github.com/rolldown/rolldown/issues/3418)
* make diagnostic message for eval build event easier to understand ([#3766](https://github.com/rolldown/rolldown/issues/3766)) ([efafb0f](https://github.com/rolldown/rolldown/commit/efafb0f9bb33623ed46a3f59fbb144a2f9572057)), closes [#3759](https://github.com/rolldown/rolldown/issues/3759)
* making return type compatible with rollup/parseAst ([#3586](https://github.com/rolldown/rolldown/issues/3586)) ([659cbd3](https://github.com/rolldown/rolldown/commit/659cbd3d42aab91d37db215b46ab1e3c8f2fd559))
* **manifestPlugin:** manifest fileds should be camelCase ([#3551](https://github.com/rolldown/rolldown/issues/3551)) ([011af34](https://github.com/rolldown/rolldown/commit/011af3468126f70e108ceeb89762ccbdebae91de))
* **mf:** add remote esm module __esModule flg ([#3569](https://github.com/rolldown/rolldown/issues/3569)) ([22d8cf4](https://github.com/rolldown/rolldown/commit/22d8cf43efd78b7aafac1a4a62ea7586e65662ee))
* minify with jsx preserve ([#3730](https://github.com/rolldown/rolldown/issues/3730)) ([d6de53c](https://github.com/rolldown/rolldown/commit/d6de53cbde6e1bac517639203bb96d8d118e1eb4))
* **minify:** disable top_level option for iife format ([#3647](https://github.com/rolldown/rolldown/issues/3647)) ([335d3d6](https://github.com/rolldown/rolldown/commit/335d3d6a6b3b45838f628958bff0735d40d2b1e5))
* **node:** add rolldown dist typing check ([#3516](https://github.com/rolldown/rolldown/issues/3516)) ([32880da](https://github.com/rolldown/rolldown/commit/32880da1ff944adc143b4461d31b708e1b67c43c))
* **node:** allow `output.minify: 'dce-only'` ([#3805](https://github.com/rolldown/rolldown/issues/3805)) ([e170e6e](https://github.com/rolldown/rolldown/commit/e170e6e354a893bd552c8cfd2d183c964d0aa7f9))
* **node:** allow updating sourcemap.debugId by plugins ([#3749](https://github.com/rolldown/rolldown/issues/3749)) ([c83cc30](https://github.com/rolldown/rolldown/commit/c83cc304eaa657710db4459f47f7d91a0572e47e))
* **node:** EmittedFile type compat ([#3745](https://github.com/rolldown/rolldown/issues/3745)) ([55ddf7d](https://github.com/rolldown/rolldown/commit/55ddf7d2aa2492458951963cdd4c82b021f470bf))
* **node:** make `in` for additional properties in OutputChunk work ([#3635](https://github.com/rolldown/rolldown/issues/3635)) ([cec60e3](https://github.com/rolldown/rolldown/commit/cec60e34e7bc7bd04bc803253153cca40a0c52d5))
* **node:** proxy moduleSideEffects for moduleInfo ([#3510](https://github.com/rolldown/rolldown/issues/3510)) ([cc2d779](https://github.com/rolldown/rolldown/commit/cc2d779d10e24b2125319e5a6b0ca77b383a6854)), closes [#2781](https://github.com/rolldown/rolldown/issues/2781)
* **node:** tsc build only emit declaration ([#3509](https://github.com/rolldown/rolldown/issues/3509)) ([9c0032c](https://github.com/rolldown/rolldown/commit/9c0032c9f72034b63637756a528ea5ba0407a273))
* NormalizedOutputOptions option ([#3582](https://github.com/rolldown/rolldown/issues/3582)) ([0bfe751](https://github.com/rolldown/rolldown/commit/0bfe751df53e2286992286b2abf2c17288527cb8))
* NormalizedOutputOptions sourcemapIgnoreList ([#3714](https://github.com/rolldown/rolldown/issues/3714)) ([34556a5](https://github.com/rolldown/rolldown/commit/34556a595ffbfc84587041fe6fe58194ab91ea54))
* prefix async to runtime helper if module has top level await  ([#3696](https://github.com/rolldown/rolldown/issues/3696)) ([c4a1aae](https://github.com/rolldown/rolldown/commit/c4a1aae63f380ba7d21de405d6503593e28d75d3)), closes [#3686](https://github.com/rolldown/rolldown/issues/3686)
* preserve this for PluginContext#emitFile ([#3683](https://github.com/rolldown/rolldown/issues/3683)) ([ca89c63](https://github.com/rolldown/rolldown/commit/ca89c63178202e85bc43d07a439891050ef56cf1)), closes [#3634](https://github.com/rolldown/rolldown/issues/3634)
* preserve this for PluginContext#getModuleInfo ([#3634](https://github.com/rolldown/rolldown/issues/3634)) ([8346be7](https://github.com/rolldown/rolldown/commit/8346be737ee073f321e799ba68225ca79a49bda0))
* regressed from renovate bot ([#3484](https://github.com/rolldown/rolldown/issues/3484)) ([d660852](https://github.com/rolldown/rolldown/commit/d66085222c625182253067a05c8b239f30467034))
* replace all global `require` to `__require` when necessary ([#3469](https://github.com/rolldown/rolldown/issues/3469)) ([cfcc101](https://github.com/rolldown/rolldown/commit/cfcc101dc6973ca1636f4cf55a88cd462f70a283))
* **rust:** dynamically importing `JSON` with `output.advancedChunks.groups` ([#3584](https://github.com/rolldown/rolldown/issues/3584)) ([a7e403e](https://github.com/rolldown/rolldown/commit/a7e403efbcd6ae65cbd511241a747afe323b8312)), closes [#3437](https://github.com/rolldown/rolldown/issues/3437) [#3343](https://github.com/rolldown/rolldown/issues/3343)
* **rust:** generate specified hash length for emitted asset filenames ([#3740](https://github.com/rolldown/rolldown/issues/3740)) ([b770d2c](https://github.com/rolldown/rolldown/commit/b770d2c50e76ef0acfe881478c4075db4f9e93b0)), closes [#3737](https://github.com/rolldown/rolldown/issues/3737)
* **rust:** only replace specified-length placeholders for the `hash` type ([#3736](https://github.com/rolldown/rolldown/issues/3736)) ([f4abf32](https://github.com/rolldown/rolldown/commit/f4abf324c0fed46742c904ac345726d028e27a64))
* should consider `void expr` as side effects free if `expr` is side effects free ([#3479](https://github.com/rolldown/rolldown/issues/3479)) ([857a285](https://github.com/rolldown/rolldown/commit/857a2853111071536c2718bfcd10d24ae954ba3e)), closes [#3478](https://github.com/rolldown/rolldown/issues/3478)
* should generate correct pattern for cross chunk symbols ([#3296](https://github.com/rolldown/rolldown/issues/3296)) ([54fd0f1](https://github.com/rolldown/rolldown/commit/54fd0f1d20b412b33cce40d7f5cd9a054716b57c))
* **splitting:** indirect external symbol ([#3405](https://github.com/rolldown/rolldown/issues/3405)) ([89db1f3](https://github.com/rolldown/rolldown/commit/89db1f3553e69e7157a11ae196a6a690ec6d1c99))
* stackblitz merge_transform_options failed ([#3732](https://github.com/rolldown/rolldown/issues/3732)) ([79b9463](https://github.com/rolldown/rolldown/commit/79b9463e1b48bb86ce4ae751a31a9228228d707e))
* stackoverflow when update cjs module meta ([#3530](https://github.com/rolldown/rolldown/issues/3530)) ([ece5e77](https://github.com/rolldown/rolldown/commit/ece5e773b52c236e15278d08560c787feb6056bb))
* suppress unresolved require error in try catch block ([#3605](https://github.com/rolldown/rolldown/issues/3605)) ([b546e53](https://github.com/rolldown/rolldown/commit/b546e53adf0d8fecbca8a9f0dddbcb44cb308a99))
* sync other moduleSideEffects value in ModuleInfo ([#3520](https://github.com/rolldown/rolldown/issues/3520)) ([f65bde3](https://github.com/rolldown/rolldown/commit/f65bde39e6c7e332ba86265a5a5c4e7d40c1ccd6)), closes [#2781](https://github.com/rolldown/rolldown/issues/2781)
* synchronize js ModuleInfo mutation to rust side ([#3518](https://github.com/rolldown/rolldown/issues/3518)) ([8922436](https://github.com/rolldown/rolldown/commit/892243653c2eb6c9f8aabb38b4279a78051dee3b)), closes [#2781](https://github.com/rolldown/rolldown/issues/2781)
* syntax error when use top level await with `strict_execution_order` option ([#3512](https://github.com/rolldown/rolldown/issues/3512)) ([558ea78](https://github.com/rolldown/rolldown/commit/558ea78f5fb0784b3806fd712a94fc1e2f249325))
* **tla:** detecting TLA in top level block scope ([#3552](https://github.com/rolldown/rolldown/issues/3552)) ([747e54d](https://github.com/rolldown/rolldown/commit/747e54d54fc7aeaf6567666b879b8c12bac89849))
* top level this substitution ([#3567](https://github.com/rolldown/rolldown/issues/3567)) ([89eac9e](https://github.com/rolldown/rolldown/commit/89eac9e18764816e422384b06df114bd3907c99b))


### Features

* `RollupLog` compat of `UNRESOLVED_IMPORT` warning ([#3711](https://github.com/rolldown/rolldown/issues/3711)) ([94f05d4](https://github.com/rolldown/rolldown/commit/94f05d470e8e1d578d55a158bfd02896a2dd2055))
* add `react`, `react-jsx` and `preserve` jsx option preset ([#3770](https://github.com/rolldown/rolldown/issues/3770)) ([248c024](https://github.com/rolldown/rolldown/commit/248c0248660e3dd27098aae167d2d3b87a1ee87a))
* add base transform options ([#3564](https://github.com/rolldown/rolldown/issues/3564)) ([5b26e29](https://github.com/rolldown/rolldown/commit/5b26e29e5eaa6a7dd9069808227d2fda8006483a))
* add better error message for resolve.alias ([#3656](https://github.com/rolldown/rolldown/issues/3656)) ([816859f](https://github.com/rolldown/rolldown/commit/816859ffb70783c63d381eec82f9a8d62e33b8d6)), closes [#3615](https://github.com/rolldown/rolldown/issues/3615)
* allow mutating `chunk.isEntry` ([#3446](https://github.com/rolldown/rolldown/issues/3446)) ([afa3746](https://github.com/rolldown/rolldown/commit/afa3746e41d5dd4b62dba900b18b0f3c654f38c8))
* assetFileNames support hashCharacters ([#3495](https://github.com/rolldown/rolldown/issues/3495)) ([56e1615](https://github.com/rolldown/rolldown/commit/56e1615c22302f98db615b6f8d46cec980c683da))
* auto merge options from tsconfig to transformOption ([#3566](https://github.com/rolldown/rolldown/issues/3566)) ([98969da](https://github.com/rolldown/rolldown/commit/98969daae460cc405f12b0d6b578138eb85523f8))
* exclude self from glob import ([#3682](https://github.com/rolldown/rolldown/issues/3682)) ([d7c9682](https://github.com/rolldown/rolldown/commit/d7c9682183819c1199865444d3b38d047dcbc066))
* export `LogOrStringHandler` type ([#3513](https://github.com/rolldown/rolldown/issues/3513)) ([3583675](https://github.com/rolldown/rolldown/commit/35836752635617ce396c547d0cb6feb19a33c2f7)), closes [#3505](https://github.com/rolldown/rolldown/issues/3505)
* export `moduleRunnerTransform` from `rolldown/experimental` ([#3801](https://github.com/rolldown/rolldown/issues/3801)) ([72c6607](https://github.com/rolldown/rolldown/commit/72c6607dd2efa7e332cc5bf813967a0822184f20))
* export hookFilter extension types ([#3562](https://github.com/rolldown/rolldown/issues/3562)) ([1e69bf0](https://github.com/rolldown/rolldown/commit/1e69bf0a19f62d06057a4f497c03b1b2650d8962))
* export more types ([#3505](https://github.com/rolldown/rolldown/issues/3505)) ([1dc9359](https://github.com/rolldown/rolldown/commit/1dc935914ae03ea201d8d3d0ba68cc91cf080d67))
* expose `transform` options ([#3780](https://github.com/rolldown/rolldown/issues/3780)) ([3302be0](https://github.com/rolldown/rolldown/commit/3302be0092f5c04da34e6c728005ee82d069cfe5))
* expose more fine-grained minify option ([#3542](https://github.com/rolldown/rolldown/issues/3542)) ([d9529e7](https://github.com/rolldown/rolldown/commit/d9529e77119a4d5396fe1682f6276997ab8c0294))
* expose sourcemapDebugIds to OutputOptions and CLI ([#3720](https://github.com/rolldown/rolldown/issues/3720)) ([380d428](https://github.com/rolldown/rolldown/commit/380d42885f92b2e9a4534983ef5d55342322738e)), closes [#2516](https://github.com/rolldown/rolldown/issues/2516)
* generate source maps for css files ([#3285](https://github.com/rolldown/rolldown/issues/3285)) ([eb0a132](https://github.com/rolldown/rolldown/commit/eb0a132eae173625fa1674cbca102a97b4811b6d)), closes [#3242](https://github.com/rolldown/rolldown/issues/3242)
* **glob_plugin:** ensure query starts with `?` ([#3699](https://github.com/rolldown/rolldown/issues/3699)) ([4c71eed](https://github.com/rolldown/rolldown/commit/4c71eed047d2ac1a26d8138ffbc5a8cc79afc70e))
* **hmr:** add `experimental#development_mode` ([#3424](https://github.com/rolldown/rolldown/issues/3424)) ([71eabea](https://github.com/rolldown/rolldown/commit/71eabeac1306089a9c4c92b6b4897e9c19f6d08d))
* **hmr:** add `generate_hmr_patch` method ([#3591](https://github.com/rolldown/rolldown/issues/3591)) ([89ca806](https://github.com/rolldown/rolldown/commit/89ca8060c59882def6349abf6ef022eda11e4775))
* **hmr:** add `HmrFinalizer` ([#3773](https://github.com/rolldown/rolldown/issues/3773)) ([4666fd5](https://github.com/rolldown/rolldown/commit/4666fd5b036992ee73354a0a8ea674fce2bb206c))
* **hmr:** add `HmrManager` ([#3662](https://github.com/rolldown/rolldown/issues/3662)) ([6e30664](https://github.com/rolldown/rolldown/commit/6e30664bf14453dcaf6f809e5a68bf3a70e7554c))
* **hmr:** add binding for `generate_hmr_patch` ([#3661](https://github.com/rolldown/rolldown/issues/3661)) ([6dc69ce](https://github.com/rolldown/rolldown/commit/6dc69ce39d4b5ca649f86da8616db579b17b88ae))
* **hmr:** add struct `HmrInfo` ([#3593](https://github.com/rolldown/rolldown/issues/3593)) ([4aa9f3f](https://github.com/rolldown/rolldown/commit/4aa9f3fc3eb5252ee059bcb27a5b20cc93014d03))
* **hmr:** compute affected modules ([#3663](https://github.com/rolldown/rolldown/issues/3663)) ([c5c81e8](https://github.com/rolldown/rolldown/commit/c5c81e8d4339192c067a79d0633c8ba43b24470b))
* **hmr:** detects defult export and shims `import.meta.hot` ([#3795](https://github.com/rolldown/rolldown/issues/3795)) ([4a3ffed](https://github.com/rolldown/rolldown/commit/4a3ffed6ba633bd71f9635936af7568b1819b6cf))
* **hmr:** ensure HMR runtime included ([#3595](https://github.com/rolldown/rolldown/issues/3595)) ([1910872](https://github.com/rolldown/rolldown/commit/1910872a4cf7e3a44a02b1f9463e2fd3ee6efdaa))
* **hmr:** fetch changed modules via `HmrModuleLoader` ([#3671](https://github.com/rolldown/rolldown/issues/3671)) ([8f3be45](https://github.com/rolldown/rolldown/commit/8f3be4530239ca3fb376329a6fcbca9a5518606a))
* **hmr:** improve handling of `export default` ([#3814](https://github.com/rolldown/rolldown/issues/3814)) ([3e70979](https://github.com/rolldown/rolldown/commit/3e70979c55434135b3d7e74e6f15ce4bd2e3c80c))
* **hmr:** introduce runtime for hmr ([#3426](https://github.com/rolldown/rolldown/issues/3426)) ([8ac5268](https://github.com/rolldown/rolldown/commit/8ac5268d65a6901b9f7b6d33ddbc3e015de6e39e))
* **hmr:** node binding for `experimental#development_mode` ([#3425](https://github.com/rolldown/rolldown/issues/3425)) ([7b70903](https://github.com/rolldown/rolldown/commit/7b70903c4c4014c411b614781323274f51b091b5))
* **hmr:** register module exports in runtime ([#3594](https://github.com/rolldown/rolldown/issues/3594)) ([3a58602](https://github.com/rolldown/rolldown/commit/3a58602ca9eb1d875f59fea0c71b4a689872f6ec))
* **hmr:** scan `import.meta.hot.accept` ([#3592](https://github.com/rolldown/rolldown/issues/3592)) ([c55dcd3](https://github.com/rolldown/rolldown/commit/c55dcd351edbc1f83b270cae5e899360ae7ed479))
* **mf:** add manifest option ([#3546](https://github.com/rolldown/rolldown/issues/3546)) ([2215304](https://github.com/rolldown/rolldown/commit/2215304aaf615669e05e2bef68d0a17c2a46c61b))
* **mf:** add pre order to the resolve_id_meta ([#3652](https://github.com/rolldown/rolldown/issues/3652)) ([23ba9ab](https://github.com/rolldown/rolldown/commit/23ba9ab92055f3353f4fe0b551676ac21f55d23a))
* **mf:** generate correct name ([#3493](https://github.com/rolldown/rolldown/issues/3493)) ([087273f](https://github.com/rolldown/rolldown/commit/087273f8213ed3da639b09a4ffc4170a71541d38))
* **mf:** support cjs shared module ([#3565](https://github.com/rolldown/rolldown/issues/3565)) ([66aa97d](https://github.com/rolldown/rolldown/commit/66aa97d9c66a3a8f1e808397e2d30d982580b284))
* **mf:** support generate remote manifest json ([#3550](https://github.com/rolldown/rolldown/issues/3550)) ([7acd618](https://github.com/rolldown/rolldown/commit/7acd618ea657d4c5dd531d8ff552013be7942f94))
* **mf:** support getPublicPath option ([#3571](https://github.com/rolldown/rolldown/issues/3571)) ([7d11485](https://github.com/rolldown/rolldown/commit/7d114853375b09a6a1f8cef8b4f3cf4aebef5793))
* **mf:** support remote expose module is cjs ([#3563](https://github.com/rolldown/rolldown/issues/3563)) ([cf442e9](https://github.com/rolldown/rolldown/commit/cf442e9ca23c3226150aa15cac6b51b73e25934f))
* **mf:** support shared modules ([#3494](https://github.com/rolldown/rolldown/issues/3494)) ([306867e](https://github.com/rolldown/rolldown/commit/306867ef8188cb9a4a976bf0c0e2dadfe9675371))
* **mf:** support webpack dst remote ([#3580](https://github.com/rolldown/rolldown/issues/3580)) ([fc7dd8c](https://github.com/rolldown/rolldown/commit/fc7dd8c08276139982957582a191269175140c73))
* **mf:** using package json version if shared module version is None ([#3514](https://github.com/rolldown/rolldown/issues/3514)) ([8e1bdab](https://github.com/rolldown/rolldown/commit/8e1bdabb98487c365038482292b5a7bd1da2c726))
* **node/dev-server:** serve output files ([#3421](https://github.com/rolldown/rolldown/issues/3421)) ([c075d4a](https://github.com/rolldown/rolldown/commit/c075d4ac8e942bdc3b9ce306a44700e91e9d36bf))
* **node/test-dev-server:** apply hmr patch after files change ([#3793](https://github.com/rolldown/rolldown/issues/3793)) ([1a86e2c](https://github.com/rolldown/rolldown/commit/1a86e2c50c26b1952829a21b40dd83057a3c3469))
* **node:** allow `sourceRoot: null` in sourcemaps ([#3694](https://github.com/rolldown/rolldown/issues/3694)) ([83a587b](https://github.com/rolldown/rolldown/commit/83a587bd9184446af7365d08bfecd406b8bfe537))
* **node:** init `test-dev-server` package ([#3419](https://github.com/rolldown/rolldown/issues/3419)) ([3cbc6f8](https://github.com/rolldown/rolldown/commit/3cbc6f8b4c8302833546df144f5f5cd7b226403e))
* **node:** make `parseAst` compatible with rollup ([#3649](https://github.com/rolldown/rolldown/issues/3649)) ([d6a3336](https://github.com/rolldown/rolldown/commit/d6a33365d1e17a040dc19aefe5a53ca4d43bb7e1)), closes [#3630](https://github.com/rolldown/rolldown/issues/3630) [#3630](https://github.com/rolldown/rolldown/issues/3630)
* **node:** remove `implements` from PluginContext types ([#3804](https://github.com/rolldown/rolldown/issues/3804)) ([c110a2a](https://github.com/rolldown/rolldown/commit/c110a2ab364b35fc89764981a098e01c1f04e175)), closes [#3802](https://github.com/rolldown/rolldown/issues/3802)
* **node:** split interface for PluginContext types ([#3802](https://github.com/rolldown/rolldown/issues/3802)) ([e71f948](https://github.com/rolldown/rolldown/commit/e71f948e439df6351946d9e4f25402f321a7dda1))
* omit keys/values in glob import if they're unused ([#3657](https://github.com/rolldown/rolldown/issues/3657)) ([a606288](https://github.com/rolldown/rolldown/commit/a6062886a0115b5a67fc4aaf799ee2e20f75e66f))
* remove unused dynamic imported chunks ([#3588](https://github.com/rolldown/rolldown/issues/3588)) ([a2fc0fc](https://github.com/rolldown/rolldown/commit/a2fc0fc94a07910f7ae409de16eb52cbc3eb00e3))
* **rolldown:** support multiple hash placeholder ([#3555](https://github.com/rolldown/rolldown/issues/3555)) ([eec8017](https://github.com/rolldown/rolldown/commit/eec80170b49b13a326d9e5382166b43fe0a79d88)), closes [#2246](https://github.com/rolldown/rolldown/issues/2246) [#3323](https://github.com/rolldown/rolldown/issues/3323)
* **rust/ecma_utils:** helpers for creating AST more easily ([#3778](https://github.com/rolldown/rolldown/issues/3778)) ([b21ce2f](https://github.com/rolldown/rolldown/commit/b21ce2f73afa422e3fc2a356a729737c357fe58a))
* **rust/hmr:** create `HmrManager` after bundling when enabling hmr ([#3787](https://github.com/rolldown/rolldown/issues/3787)) ([09f27bf](https://github.com/rolldown/rolldown/commit/09f27bf543650f6fab679d8140f6e590b104504b))
* **rust/hmr:** improve runtime and generate runnable patch ([#3792](https://github.com/rolldown/rolldown/issues/3792)) ([4511b29](https://github.com/rolldown/rolldown/commit/4511b29215d6ac4ef3739b6cb8ed2517e5cd2109))
* **rust/hmr:** rewrite import declaration ([#3779](https://github.com/rolldown/rolldown/issues/3779)) ([f0c743f](https://github.com/rolldown/rolldown/commit/f0c743ffa78d02089d8e10e9ba6d99706d959fb7))
* **rust:** add normalModule to module_parsed hook ([#3560](https://github.com/rolldown/rolldown/issues/3560)) ([964a30d](https://github.com/rolldown/rolldown/commit/964a30d0bf9e1897a0813aa59fe09199f937658e))
* support `import.meta.ROLLUP_FILE_URL_(referenceId)` ([#3488](https://github.com/rolldown/rolldown/issues/3488)) ([18af1ba](https://github.com/rolldown/rolldown/commit/18af1ba0435cf0c4447bdf9e1357d69065b84714))
* support `require` with a `TemplateLiteral` argument ([#3450](https://github.com/rolldown/rolldown/issues/3450)) ([d8624bb](https://github.com/rolldown/rolldown/commit/d8624bbabfe1353e85ee8984544da1b2f94a4c33)), closes [#3447](https://github.com/rolldown/rolldown/issues/3447)
* support negated glob import patterns ([#3624](https://github.com/rolldown/rolldown/issues/3624)) ([9df23df](https://github.com/rolldown/rolldown/commit/9df23df3bc849004cb634b1b8977b046e5c9d734))
* support RenderedModule.renderedExports ([#3633](https://github.com/rolldown/rolldown/issues/3633)) ([ba68200](https://github.com/rolldown/rolldown/commit/ba682007f94534124ff38d69e6dae910ccb7965a))
* support sanitizeFileName ([#3418](https://github.com/rolldown/rolldown/issues/3418)) ([1abfb8b](https://github.com/rolldown/rolldown/commit/1abfb8bb9a9533e16499510f50eaacf2a323fbe2))
* **tla:** await `init` func generated by `__toESM` ([#3537](https://github.com/rolldown/rolldown/issues/3537)) ([fcdbd44](https://github.com/rolldown/rolldown/commit/fcdbd440b7844a681046d649ef82bbc52866de5f))
* **tla:** remove redundant `await` before `__toESM` ([#3538](https://github.com/rolldown/rolldown/issues/3538)) ([deaa40c](https://github.com/rolldown/rolldown/commit/deaa40cfc01d6e89331b8e9c6a8d02c28b54601e))
* **tla:** support TLA while enabling strict execution order ([#3528](https://github.com/rolldown/rolldown/issues/3528)) ([9c3c2e6](https://github.com/rolldown/rolldown/commit/9c3c2e6512ff699d1998ca19e32c39b84e8d717f))
* treeshake.manualPureFunctions ([#3789](https://github.com/rolldown/rolldown/issues/3789)) ([7b0c517](https://github.com/rolldown/rolldown/commit/7b0c517d0e84e5954019698452b3d537c9a1e9f9))
* treeshake.unknownGlobalSideEffects ([#3790](https://github.com/rolldown/rolldown/issues/3790)) ([9b28db1](https://github.com/rolldown/rolldown/commit/9b28db1e797fb7f7e1b17761a13f0637a6604b6a))
* validate option ([#3748](https://github.com/rolldown/rolldown/issues/3748)) ([b626079](https://github.com/rolldown/rolldown/commit/b62607931924ffbb21f33be68d7e630f3ade4dcd))
* validate transform options ([#3781](https://github.com/rolldown/rolldown/issues/3781)) ([c6d591b](https://github.com/rolldown/rolldown/commit/c6d591be0e9f64333049297a549a2bade85972bd))
* warn when a file is emitted with the same name ([#3503](https://github.com/rolldown/rolldown/issues/3503)) ([4bb775f](https://github.com/rolldown/rolldown/commit/4bb775f37ca8819509b8aa2e9ce5ad0302dabce9))
* **watcher:** add buildDelay option ([#3608](https://github.com/rolldown/rolldown/issues/3608)) ([2cb8efb](https://github.com/rolldown/rolldown/commit/2cb8efb9cacaf97e6a654e91296235f2ee868dae))
* **watcher:** debounce invalidate ([#3607](https://github.com/rolldown/rolldown/issues/3607)) ([b17487d](https://github.com/rolldown/rolldown/commit/b17487de653e95fb5d236055c218c4b0a2c5281d))
* wildcard alias ([#3617](https://github.com/rolldown/rolldown/issues/3617)) ([4b46c6e](https://github.com/rolldown/rolldown/commit/4b46c6ee22a6df82834ef20491331739ce80a6b7))


### Performance Improvements

* **advanced_chunks:** remove unnecessary sorting ([#3655](https://github.com/rolldown/rolldown/issues/3655)) ([b814905](https://github.com/rolldown/rolldown/commit/b814905caa84034b7bfde5bdc0506dea49847e68))
* avoid unnecessary alloc in to_base64 ([#3462](https://github.com/rolldown/rolldown/issues/3462)) ([92d10c3](https://github.com/rolldown/rolldown/commit/92d10c3224461ed16bc3c6b0b04c05943b6b50fe))
* **binding:** modules property in RenderedChunk ([#3533](https://github.com/rolldown/rolldown/issues/3533)) ([06b6bbf](https://github.com/rolldown/rolldown/commit/06b6bbf183998e1ed962a29075ae0da105997504))
* **plugin_dynamic_import_vars:** remove Regex ([#3702](https://github.com/rolldown/rolldown/issues/3702)) ([3337767](https://github.com/rolldown/rolldown/commit/33377678483b33399f7fd4e5bddfb183a4a37843))
* reduce clone of transform_options ([#3782](https://github.com/rolldown/rolldown/issues/3782)) ([73fa972](https://github.com/rolldown/rolldown/commit/73fa972d814336b73a525ba421f39c0066c1e056))
* reduce memory alloc for StmtInfo ([#3590](https://github.com/rolldown/rolldown/issues/3590)) ([b0898a8](https://github.com/rolldown/rolldown/commit/b0898a8695d8ec4d0ea29577a7b204d84694f3fb))
* remove unnecessary to_string ([#3715](https://github.com/rolldown/rolldown/issues/3715)) ([0ba0ac4](https://github.com/rolldown/rolldown/commit/0ba0ac4c2412c689278bb22ea24f2d999ad0be0b))
* replace data url parse with nom ([#3677](https://github.com/rolldown/rolldown/issues/3677)) ([ca90fa2](https://github.com/rolldown/rolldown/commit/ca90fa2eea5ffc062cef2f16f4b2bc8f02ac3a80))
* **rolldown_plugin_replace:** replace static Regex with pure function ([#3690](https://github.com/rolldown/rolldown/issues/3690)) ([e1deaff](https://github.com/rolldown/rolldown/commit/e1deaff13a5604a264db0ef275923606ebc75f18))
* **rolldown_utils:** remove replace placeholder Regex ([#3691](https://github.com/rolldown/rolldown/issues/3691)) ([582886c](https://github.com/rolldown/rolldown/commit/582886c9dfb8ae356e8a84c45f06124ea90fec7d))
* **rust:** avoid emitted_filenames to hash twice ([#3504](https://github.com/rolldown/rolldown/issues/3504)) ([9404df1](https://github.com/rolldown/rolldown/commit/9404df14656a2e16f7d427bf865877f7fa7b0d78))
* **rust:** avoid unnecessary `into_owned` on `Cow<str>` ([#3470](https://github.com/rolldown/rolldown/issues/3470)) ([cdeedac](https://github.com/rolldown/rolldown/commit/cdeedacc6fc7ac10ecb41246072c5262b59d4efd))
* **rust:** avoid unnecessary source clone ([#3815](https://github.com/rolldown/rolldown/issues/3815)) ([a22cfd2](https://github.com/rolldown/rolldown/commit/a22cfd20269b6d1606fc715522bbbebbcdc7a684))
* **rust:** improve `get_lived_entry` ([#3665](https://github.com/rolldown/rolldown/issues/3665)) ([6e16c3c](https://github.com/rolldown/rolldown/commit/6e16c3c605978dbe4d8e6878799b78de204f0b4d))
* should not invalidate ModuleOptions if mutate `ModuleInfo` has same id with hook param ([#3519](https://github.com/rolldown/rolldown/issues/3519)) ([370af62](https://github.com/rolldown/rolldown/commit/370af620d44f342c6f2b35408f3486736d02a584))



# [1.0.0-beta.3](https://github.com/rolldown/rolldown/compare/v1.0.0-beta.2...v1.0.0-beta.3) (2025-01-23)


### Bug Fixes

* dynamic tree shaking more await syntax ([#3399](https://github.com/rolldown/rolldown/issues/3399)) ([f11bf03](https://github.com/rolldown/rolldown/commit/f11bf037fc427abe939431d960fb32d8ce5fad29)), closes [#3396](https://github.com/rolldown/rolldown/issues/3396)
* force cjs wrapper to be included in the output ([#3400](https://github.com/rolldown/rolldown/issues/3400)) ([8a5eba0](https://github.com/rolldown/rolldown/commit/8a5eba040b5be2b446b50f9f28d054607e95954b))
* should not set default value for treeshake in cli normalization ([#3398](https://github.com/rolldown/rolldown/issues/3398)) ([72ddc66](https://github.com/rolldown/rolldown/commit/72ddc66261fc660acbeeb658e8290850909147fd)), closes [#3392](https://github.com/rolldown/rolldown/issues/3392)
* shouldn't generate unused `__toESM` calls ([#3401](https://github.com/rolldown/rolldown/issues/3401)) ([5deb279](https://github.com/rolldown/rolldown/commit/5deb2793fc9176d31796f224645dccd3890fa1dc))
* sort function panic due to user-provided comparison function does not correctly implement a total order  ([#3384](https://github.com/rolldown/rolldown/issues/3384)) ([4986046](https://github.com/rolldown/rolldown/commit/4986046f594c5fb6663ca7b5bf31663d4e35903e))


### Features

* **mf:** support runtime plugin ([#3377](https://github.com/rolldown/rolldown/issues/3377)) ([83cd091](https://github.com/rolldown/rolldown/commit/83cd091b5ab95b4d949564330e8b59fc3260c89c))
* PluginContext.getFileName using emitted chunk reference id ([#3409](https://github.com/rolldown/rolldown/issues/3409)) ([9db3240](https://github.com/rolldown/rolldown/commit/9db3240ead7d63651b38ac3ec7495e83c07e1c8f))
* support function for assetFilenames ([#3397](https://github.com/rolldown/rolldown/issues/3397)) ([242f007](https://github.com/rolldown/rolldown/commit/242f007a5582a620fe54cba0c3ab200649f325ce))



# [1.0.0-beta.2](https://github.com/rolldown/rolldown/compare/v1.0.0-beta.1...v1.0.0-beta.2) (2025-01-20)


### Bug Fixes

* bailout module.exports for eliminate interop default property access ([#3369](https://github.com/rolldown/rolldown/issues/3369)) ([4f55359](https://github.com/rolldown/rolldown/commit/4f553596cbf103169546e8c8c1f15368ed262640)), closes [#3364](https://github.com/rolldown/rolldown/issues/3364)
* **cjs:** named exports if module wrapped ([#3235](https://github.com/rolldown/rolldown/issues/3235)) ([ee80c98](https://github.com/rolldown/rolldown/commit/ee80c98085d36c2949d9ee4734ac900e0b96e68a))
* **cli:** allow load ts config file that are not in the working directory ([#3238](https://github.com/rolldown/rolldown/issues/3238)) ([fd29aff](https://github.com/rolldown/rolldown/commit/fd29affd53f3c641c5ed1e38a709cfbe2bba3e46)), closes [#3237](https://github.com/rolldown/rolldown/issues/3237)
* **cli:** call options hook once with multiply output ([#3348](https://github.com/rolldown/rolldown/issues/3348)) ([ae34f38](https://github.com/rolldown/rolldown/commit/ae34f383870acad76e829fc1ab6700b7723ef59b))
* **cli:** remove config default value ([#3307](https://github.com/rolldown/rolldown/issues/3307)) ([bcb0b35](https://github.com/rolldown/rolldown/commit/bcb0b35fda735a1266378e00854d87b54fbe7e9d))
* dynamic require by unused function ([#3271](https://github.com/rolldown/rolldown/issues/3271)) ([d07e45c](https://github.com/rolldown/rolldown/commit/d07e45cce088f0f5acee935c496f01797e89094f)), closes [#3268](https://github.com/rolldown/rolldown/issues/3268) [/github.com/evanw/esbuild/blob/d34e79e2a998c21bb71d57b92b0017ca11756912/internal/linker/linker.go#L3220-L3245](https://github.com//github.com/evanw/esbuild/blob/d34e79e2a998c21bb71d57b92b0017ca11756912/internal/linker/linker.go/issues/L3220-L3245)
* extract `RestElement` in `ObjectPattern`  ([#3374](https://github.com/rolldown/rolldown/issues/3374)) ([a65dcfe](https://github.com/rolldown/rolldown/commit/a65dcfedf5d6f2c52fbd173131ed5284567eb82f))
* **live-bindings:** remove unnecessary bindings for the default export decl ([#3276](https://github.com/rolldown/rolldown/issues/3276)) ([2cb364b](https://github.com/rolldown/rolldown/commit/2cb364b30c36284a2887803eb648ca427d0acf9d))
* make outdir behavior compatible with rollup if file is used ([#3304](https://github.com/rolldown/rolldown/issues/3304)) ([a1fb0ff](https://github.com/rolldown/rolldown/commit/a1fb0ffbcac5259a3b78a4d8dfa46b9a5862827c))
* **plugin/json:** tree shake json named exports ([#3346](https://github.com/rolldown/rolldown/issues/3346)) ([b8f0e19](https://github.com/rolldown/rolldown/commit/b8f0e19b2e6b0024e61ae07496a0f988bc195925))
* run just setup limited node version ([#3305](https://github.com/rolldown/rolldown/issues/3305)) ([14867ce](https://github.com/rolldown/rolldown/commit/14867ce9fdde0ec50692c4c2d77f0072ebce1018))
* **splitting:** ensure correct __export symbol reference ([#3289](https://github.com/rolldown/rolldown/issues/3289)) ([31cc7b8](https://github.com/rolldown/rolldown/commit/31cc7b82e265ae53797ffabf7dda4c02a1c12b8b))
* **umd:** should render `exports` correctly ([#3270](https://github.com/rolldown/rolldown/issues/3270)) ([6da18a6](https://github.com/rolldown/rolldown/commit/6da18a67dc6f6713ba519812eeb949496893f5a2)), closes [#3269](https://github.com/rolldown/rolldown/issues/3269)
* **watcher:** respect file option for BundleEndEventData#output ([#3308](https://github.com/rolldown/rolldown/issues/3308)) ([09a6157](https://github.com/rolldown/rolldown/commit/09a6157e8a0aa5652c294fc940931d6202d4c765))


### Features

* add rolldown_plugin_module_federation crate ([#3328](https://github.com/rolldown/rolldown/issues/3328)) ([d046b47](https://github.com/rolldown/rolldown/commit/d046b47ef9af6900bb5a0b88cdb2013db4147c8d))
* **advanced_chunks:** support `max_size` option ([#3383](https://github.com/rolldown/rolldown/issues/3383)) ([71c7e93](https://github.com/rolldown/rolldown/commit/71c7e93ae98dde8eb1bf3b0b8639abd49e7b28b3))
* **advanced-chunks:** support `minModuleSize` and `maxModuleSize` options ([#3361](https://github.com/rolldown/rolldown/issues/3361)) ([1c2ef25](https://github.com/rolldown/rolldown/commit/1c2ef25de32a0d49b2bbfa4b3a0050a7d3c578a2))
* basic cache ([#3339](https://github.com/rolldown/rolldown/issues/3339)) ([9e86366](https://github.com/rolldown/rolldown/commit/9e86366dffc3cc740541cd0c1d53bafe3441cc9d))
* **cli:** resolve `rolldown.config` by default when `-c` is unspecified ([#3250](https://github.com/rolldown/rolldown/issues/3250)) ([f90856a](https://github.com/rolldown/rolldown/commit/f90856ad7b4a028369dcd2627c860343910d1333)), closes [#3248](https://github.com/rolldown/rolldown/issues/3248)
* eliminate unnecessary default property access ([#3312](https://github.com/rolldown/rolldown/issues/3312)) ([cf4c254](https://github.com/rolldown/rolldown/commit/cf4c254ef613ca8dca381ca1831d9f9c122e3d13)), closes [1#L191-L201](https://github.com/1/issues/L191-L201)
* esm_flag analyzer ([#3257](https://github.com/rolldown/rolldown/issues/3257)) ([3484a68](https://github.com/rolldown/rolldown/commit/3484a686c6aaa883682ab83e77fbe5ddc4005597))
* export `RolldownPluginOption` type ([#3360](https://github.com/rolldown/rolldown/issues/3360)) ([f24e496](https://github.com/rolldown/rolldown/commit/f24e4969c4849c4b8d3e3fb2d29ee0977f5ca7ce))
* export parseAst and parseAstAsync from 'rolldown/parseAst' ([#3208](https://github.com/rolldown/rolldown/issues/3208)) ([b2a06cb](https://github.com/rolldown/rolldown/commit/b2a06cbf810a3e411bb6aa51c471f24b7c22844d))
* give error when using both the file and the dir option ([#3282](https://github.com/rolldown/rolldown/issues/3282)) ([82b4e30](https://github.com/rolldown/rolldown/commit/82b4e3062abaa7e4acf3ee1eea0553c7fd8ef13a))
* merge generated `__toESM` calls of cjs modules ([#3343](https://github.com/rolldown/rolldown/issues/3343)) ([400b5e4](https://github.com/rolldown/rolldown/commit/400b5e4a3c7955feab5833bb287c36fb82c7ebbf))
* **mf:** add init host code for entry ([#3365](https://github.com/rolldown/rolldown/issues/3365)) ([fc9d5a5](https://github.com/rolldown/rolldown/commit/fc9d5a5833ea5381af6d1da26de2d967c32bcd11))
* **mf:** add option ([#3329](https://github.com/rolldown/rolldown/issues/3329)) ([72766d5](https://github.com/rolldown/rolldown/commit/72766d543b6836a93b3fd63d7dcd71e59499ecc6))
* **mf:** generate remote entry ([#3330](https://github.com/rolldown/rolldown/issues/3330)) ([5cf14c3](https://github.com/rolldown/rolldown/commit/5cf14c352a23865a9d4d3709687935a903997900))
* **mf:** load remote entry module ([#3366](https://github.com/rolldown/rolldown/issues/3366)) ([5bb588b](https://github.com/rolldown/rolldown/commit/5bb588b1002a7c54ceae10a63bfd8f8935c4c68d))
* **node:** expose `maxSize` and `maxModuleSize` options ([#3386](https://github.com/rolldown/rolldown/issues/3386)) ([0be6271](https://github.com/rolldown/rolldown/commit/0be627144b29f4c8b90bb1701ac4a381f9081421))
* **node:** expose moduleFederationPlugin ([#3352](https://github.com/rolldown/rolldown/issues/3352)) ([abc2c0a](https://github.com/rolldown/rolldown/commit/abc2c0a619d61f84b188ad0f7f87871176a88b79))
* **node:** support `output.target` ([#3254](https://github.com/rolldown/rolldown/issues/3254)) ([b123256](https://github.com/rolldown/rolldown/commit/b1232566d6db04d06b55cedfee27ba0cca63e568)), closes [#3252](https://github.com/rolldown/rolldown/issues/3252)
* **string_wizard:** add `.hasChanged()` ([#3379](https://github.com/rolldown/rolldown/issues/3379)) ([0d33eef](https://github.com/rolldown/rolldown/commit/0d33eef53148a166b287b99770cdd08fc73ee49f)), closes [/github.com/nuxt/nuxt/pull/30066#issuecomment-2532929079](https://github.com//github.com/nuxt/nuxt/pull/30066/issues/issuecomment-2532929079)
* support emit chunk at buildStart hook ([#3357](https://github.com/rolldown/rolldown/issues/3357)) ([8c96981](https://github.com/rolldown/rolldown/commit/8c96981fb5e4a8a828cad02edf598dae1e60f306)), closes [/github.com/rolldown/rolldown/pull/3330/files#r1915910715](https://github.com//github.com/rolldown/rolldown/pull/3330/files/issues/r1915910715)
* support PluginContext emitChunk ([#3327](https://github.com/rolldown/rolldown/issues/3327)) ([dc3fc06](https://github.com/rolldown/rolldown/commit/dc3fc06bccdda2d342c2da25c2e6bfee93b8d9cb)), closes [/github.com/rollup/rollup/blob/master/src/Chunk.ts#L420](https://github.com//github.com/rollup/rollup/blob/master/src/Chunk.ts/issues/L420)


### Performance Improvements

* `into_binding_chunk_modules` when chunk is large  ([#3267](https://github.com/rolldown/rolldown/issues/3267)) ([c7582a6](https://github.com/rolldown/rolldown/commit/c7582a6a8848c0281897fb672a05d0ad67e82939))
* cache raw_source and module_type ([#3341](https://github.com/rolldown/rolldown/issues/3341)) ([6d849ea](https://github.com/rolldown/rolldown/commit/6d849eaa8f567e187769fddd3d79057f87a530ca))
* improve hook filter related logic in `rolldown_binding` ([#3266](https://github.com/rolldown/rolldown/issues/3266)) ([65e3d8a](https://github.com/rolldown/rolldown/commit/65e3d8ad8d736fc23feb6edf0a911f9505dee529)), closes [#3265](https://github.com/rolldown/rolldown/issues/3265)
* **link_stage:** improve `LinkStage#sort_modules` ([#3318](https://github.com/rolldown/rolldown/issues/3318)) ([8bb8e93](https://github.com/rolldown/rolldown/commit/8bb8e93c9dddd911d3781ebba2571b996537799a))
* **link_stage:** improve `LinkStage#wrap_modules` ([#3321](https://github.com/rolldown/rolldown/issues/3321)) ([35fb1f8](https://github.com/rolldown/rolldown/commit/35fb1f881c48fcabe7137ba20e90d02d58cd3c5e)), closes [/github.com/rolldown/rolldown/blob/42ee4c997588b2ee205eceae36b48a18c95aed96/crates/rolldown/src/stages/link_stage/wrapping.rs#L89-L101](https://github.com//github.com/rolldown/rolldown/blob/42ee4c997588b2ee205eceae36b48a18c95aed96/crates/rolldown/src/stages/link_stage/wrapping.rs/issues/L89-L101)
* parallel `to_module_info` ([#3293](https://github.com/rolldown/rolldown/issues/3293)) ([5571513](https://github.com/rolldown/rolldown/commit/5571513ecf6c6e642e9e7b588acf1b299f38d43c))
