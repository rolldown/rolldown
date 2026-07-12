# Frozen generated support

The scale corpus uses every tracked file from each exact repository pin plus 15 small generated support entries. Matrix startup hashes both sets. Preparation never installs dependencies, invokes Nuxt, starts Vite, or includes support generation in build timing; it copies the committed overlays and creates two fixed relative workspace symlinks.

The generated support aggregate is 15 entries, 28,403 bytes, two symlinks, SHA-256 `64370492a4d453788b0b6ef0134218814e192fcefe6d1dd4bc3f7264f3457c48` under the same path/kind/bytes/content-hash record algorithm as tracked support.

## PrimeVue Volt

Source lock: `pnpm-lock.yaml` SHA-256 `6bf8aa0f1f3be17634ec7d381141b623cf0620f73b789c8f6107cbdba4152238` at commit `d4374cb7c1267f35eba7cee5d0a266f50ca8ec84`.

Capture used Node.js 24.18.0 and pnpm 9.6.0:

```sh
corepack pnpm@9.6.0 install --frozen-lockfile --filter 'volt...'
```

The `volt` postinstall runs `nuxt prepare`. The committed `apps/volt/.nuxt/tsconfig.json` is 7,837 bytes with SHA-256 `3156d237f0492a859cc46b69f148aa6de245b6ef9f09ead362877805e3ed450e`. Other generated Nuxt files and installed packages are not copied into the benchmark corpus.

## Element Plus workspace links

Source lock: `pnpm-lock.yaml` SHA-256 `ce35683ddb2d43f5b11d08cf0037938b8b513ae087b34d127c6a3e3e830cbedb` at commit `85bdf740c1d550f3ca44472262e2a314039eab7d`.

The admission diagnosis used Node.js 24.18.0 and pnpm 11.11.0:

```sh
corepack pnpm@11.11.0 install --frozen-lockfile --ignore-scripts
```

Only the workspace resolution needed by compiler-sfc is materialized during corpus preparation: `node_modules/@element-plus/components -> ../../packages/components` and `node_modules/@element-plus/hooks -> ../../packages/hooks`. Their exact targets are hashed as symlink contents. No installed dependency tree is copied.

## Vuestic generated configs

Source lock: `yarn.lock` SHA-256 `76a5e3b35cf6f3c92769966552c4458456afa07b4c2b21b9756ff90508554d6f` at commit `c5337ed8e7e24ea294221326fe2ca6af8d3b8e1b`.

Capture used Node.js 24.18.0 and Yarn 4.9.1. The sandbox's tracked `scripts/stud-nuxt.mjs` writes its empty generated config. The docs config was captured from the pinned Nuxt environment. The pinned source's REPL module currently expects `style.css` although its first Vite pass emits `vuestic-ui.css`, so the exact capture sequence was:

```sh
corepack yarn@4.9.1 install --immutable
corepack yarn@4.9.1 workspace sandbox dependenciesInstalled
corepack yarn@4.9.1 workspace docs nuxi prepare
cp packages/docs/public/vuestic-out/vuestic-ui.css packages/docs/public/vuestic-out/style.css
corepack yarn@4.9.1 workspace docs nuxi prepare
```

The first docs prepare is expected to stop at the missing `style.css` after emitting `vuestic-ui.css`; only the final `packages/docs/.nuxt/tsconfig.json` is retained. The committed docs config is 7,553 bytes with SHA-256 `d93eeb2ef060ddeb7d0c7554a37b0ca925f680b81480feff682addd5cff2ebee`; the sandbox config is three bytes including its committed newline with SHA-256 `ca3d163bab055381827226140568f3bef7eaac187cebd76878e0b63e9e442356`.

The nested compiler playground is not a Yarn workspace. Its two extended config packages are therefore frozen independently from exact MIT registry artifacts:

- `@vue/tsconfig@0.5.1`, registry integrity `sha512-VcZK7MvpjuTPx2w6blwnwZAu5/LgBUtejFOi3pPGQFXQN5Ela03FUtd2Qtg4yWGGissVL0dr6Ro1LfOFh+PCuQ==`.
- `@tsconfig/node20@20.1.9`, registry integrity `sha512-IjlTv1RsvnPtUcjTqtVsZExKVq+KQx4g5pCP5tI7rAs6Xesl2qFwSz/tPDBC4JajkL/MlezBu3gPUwqRHl+RIg==`.

Their package metadata, licenses, readmes, and config files are committed under `support-overlays/` with repository newline normalization and separately pinned byte hashes; preparation copies them to the original `node_modules` lookup paths. The benchmark does not contact the registry.

## Admission boundary

The original 4,540 sources compile under the ordinary unplugin-vue adapter with only these 15 entries added to the tracked support snapshot. Quasar contributes 1,110 admitted content-unique sources. Ten otherwise parse-eligible Quasar SFCs using `template lang="pug"` are excluded because this curve freezes the default HTML template compiler, not optional preprocessors. Three Quasar TypeScript playground paths are excluded because their tracked tsconfig extends missing generated `.quasar/tsconfig.json`; one excluded content has a byte-identical tracked template copy under a different path that remains eligible and compiles from its own support context. `run-admission-audit.mjs` makes this boundary executable: it selects the first UTF-8-sorted path for each distinct content among the structurally eligible tracked Quasar SFCs, compiles all 1,112 selected sources before the three exclusions, requires exactly those three failures, then compiles and generates all 5,650 retained content-unique sources. The pre-exclusion selection is 2,245,255 bytes with aggregate SHA-256 `49f5089abac134b76c7e9ee6e21db1c073ebcdf85e1cd46d65c3ef82fe36945d`. The raw report embeds the current harness source manifest and resolved Vue package artifacts, and a compact evidence file pins its SHA-256. Formal execution requires both raw and compact files to be tracked and byte-identical to `HEAD`. Neither phase installs Quasar dependencies or records performance fields.

## Superseded schema 1 diagnosis

The original tracked-only 4,540-source snapshot failed ordinary compile admission with 717 top-level transform errors. The repository split was PrimeVue 601, Vuestic UI 103, Element Plus 13, and zero for TDesign Vue Next. Error signatures were 704 Vite Oxc `TSCONFIG_ERROR` diagnostics and 13 compiler-sfc unresolved-extends diagnostics.

The 704 tsconfig failures split by nearest root as follows:

- PrimeVue `apps/volt/tsconfig.json`: 601; its tracked config extends missing generated `apps/volt/.nuxt/tsconfig.json`.
- Vuestic `packages/docs/tsconfig.json`: 98; it extends missing generated `packages/docs/.nuxt/tsconfig.json`.
- Vuestic `packages/compiler/playground/tsconfig.json`: 4; its referenced configs extend missing package configs `@vue/tsconfig/tsconfig.dom.json` and `@tsconfig/node20/tsconfig.json`.
- Vuestic `packages/sandbox/tsconfig.json`: 1; it extends missing generated `packages/sandbox/src/.nuxt/tsconfig.json`, which the repository's tracked `scripts/stud-nuxt.mjs` writes as `{}`.

The 13 Element Plus failures were unresolved workspace type imports: `@element-plus/hooks` seven, `@element-plus/components/input` two, `@element-plus/components/popper` two, `@element-plus/components/dialog` one, and `@element-plus/components/cascader-panel` one.

The old 32-source prefix had exactly six failures, all under PrimeVue Volt:

- `primevue/apps/volt/doc/avatar/DownloadDoc.vue`
- `primevue/apps/volt/doc/button/DisabledDoc.vue`
- `primevue/apps/volt/doc/checkbox/BasicDoc.vue`
- `primevue/apps/volt/doc/checkbox/IndeterminateDoc.vue`
- `primevue/apps/volt/doc/menu/BasicDoc.vue`
- `primevue/apps/volt/volt/Card.vue`

Compile-only controls separated source behavior from environment setup. Exact pinned Element workspace installation made all 13 Element failures pass; PrimeVue installation plus its `nuxt prepare` postinstall made all 601 Prime failures pass; full pinned Vuestic installation, its generated configs, and the two exact standalone config packages made all 103 Vuestic failures pass. Copying only the 15 frozen support entries then admitted all original 4,540 sources together. The failure was therefore missing static support material, not a requirement to run Nuxt or Vite during each SFC transform or benchmark case.
