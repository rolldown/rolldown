# CLI Design

The CLI uses [cac](https://github.com/cacjs/cac) (v6.7.14) for argument parsing. cac is the same library used by Vite and tsdown.

## Pipeline

```
bin/cli.mjs
  → src/cli/index.ts (entry)
    → checkNodeVersion()
    → parseCliArguments()
      → arguments/index.ts
        → getCliSchemaInfo()                 — flatten valibot schema into { key: { type, description } }
        → build `options` export             — for help.ts consumption (kebab-case keys)
        → build knownKeys / shortAliases     — for post-processing
        → register options with cac          — loop schemaInfo + alias, build rawName strings
        → cli.parse(process.argv, { run: true })
        → post-processing:
          → delete `--` key and short-alias duplicates
          → prototype pollution guard
          → unknown option detection + warning
          → rawArgs snapshot
          → remove unknown keys
          → type coercion (duplicate filtering + array wrapping)
          → object option parsing (key=val,key=val)
      → arguments/normalize.ts
        → validateCliOptions() via valibot
        → split into input/output based on schema keys
        → merge positionals into input.input
    → process --environment (KEY:VALUE → process.env)
    → if --help: showHelp()
    → if --version: print version
    → if --config: bundleWithConfig(configPath, cliOptions, rawArgs)
    → if input specified: bundleWithCliOptions(cliOptions)
    → else: showHelp()
```

## Key Files

| File                              | Role                                                                              |
| --------------------------------- | --------------------------------------------------------------------------------- |
| `cli/index.ts`                    | Entry point — orchestrates the pipeline                                           |
| `cli/arguments/index.ts`          | Core parsing — cac setup, option registration, post-processing                    |
| `cli/arguments/normalize.ts`      | Splits flat options into `input`/`output`, validates with valibot                 |
| `cli/arguments/alias.ts`          | Short flags, `reverse`, `requireValue`, `hint` config                             |
| `cli/arguments/utils.ts`          | `setNestedProperty`, `camelCaseToKebabCase`                                       |
| `cli/commands/help.ts`            | Custom help text generation (reads `options` export)                              |
| `cli/commands/bundle.ts`          | `bundleWithConfig`, `bundleWithCliOptions`, watch mode                            |
| `cli/logger.ts`                   | consola logger, replaced with plain `console.log` when `ROLLDOWN_TEST=1`          |
| `utils/validator.ts`              | valibot schemas for all CLI options, `getCliSchemaInfo()`, input/output key lists |
| `utils/flatten-valibot-schema.ts` | Recursively flattens valibot object schemas into `{ key: { type, description } }` |

## What `parseCliArguments()` Returns

```ts
interface NormalizedCliOptions {
  input: InputOptions;
  output: OutputOptions;
  help: boolean;
  config: string;
  version: boolean;
  watch: boolean;
  environment?: string | string[];
}

// Plus rawArgs: Record<string, any> — all parsed args including unknown ones
```

## cac Setup

### Option Registration

Loop over `schemaInfo` + `alias` and register each option with cac. Schema keys are camelCase (e.g. `moduleTypes`); cac's internal `camelcaseOptionName` handles kebab↔camel conversion, so we register with the camelCase key directly. cac will match both `--moduleTypes` and `--module-types` from argv.

```ts
for (const [key, info] of Object.entries(schemaInfo)) {
  const config = alias[key as keyof typeof alias];

  let rawName = '';
  if (config?.abbreviation) rawName += `-${config.abbreviation}, `;

  if (config?.reverse) {
    rawName += `--no-${key}`;
  } else {
    rawName += `--${key}`;
  }

  // Bracket syntax determines how cac handles the option:
  // - No brackets → boolean (registered in mri's boolean list)
  // - <required>  → string, checkOptionValue throws CACError if missing
  // - [optional]  → string, returns true if no value follows
  if (info.type !== 'boolean' && !config?.reverse) {
    if (config?.requireValue) {
      rawName += ` <${config?.hint ?? key}>`;
    } else {
      rawName += ` [${config?.hint ?? key}]`;
    }
  }

  cli.option(rawName, info.description ?? config?.description ?? '');
}
```

### Default Command

```ts
const cmd = cli.command('[...input]', '');
cmd.allowUnknownOptions();    // suppress cac's unknown option error — we warn instead
cmd.ignoreOptionDefaultValue(); // prevent cac from injecting --no-* defaults
cmd.action((input, opts) => { ... });
cli.parse(process.argv, { run: true });
```

### What cac Gives Us

- camelCase/kebab-case interchangeable matching (fixes [#8410])
- `--no-*` boolean negation
- `<required>` value validation via `checkOptionValue()` — throws `CACError`
- `[optional]` value parsing — fixes `-s inline` position restriction ([#3248])
- Dot-notation nesting via `setDotProp` (`--transform.define X=Y` → `{ transform: { define: 'X=Y' } }`)
- Short flag aliases and stacking (`-ms` = `--minify --sourcemap`)
- Array auto-accumulation for repeated flags

### What We Implement Ourselves

- **Object parsing** — `--module-types .a=text,.b=json`: split on `,` then `=`. Supports both comma-separated single flag and repeated flags.
- **Unknown option warning** — `allowUnknownOptions()` suppresses cac's error; we detect and warn with our own message format.
- **Prototype pollution guard** — cac's `setDotProp` doesn't guard against `__proto__`, `constructor`, `prototype`.
- **Input/output splitting** — rolldown-specific logic in `normalize.ts` that splits flat options into `InputOptions` and `OutputOptions`.
- **Custom help text** — don't use `cli.help()`; keep our custom generator with sorting, padding, examples, notes.
- **Duplicate option filtering** — take last value for non-array types; keep arrays for `external` and `input`.
- **rawArgs assembly** — snapshot of all parsed args (including unknown) for config function passthrough.
- **Short-alias key cleanup** — mri duplicates both short and long names (e.g. `{ s: true, sourcemap: true }`); we delete the short keys.

## Post-Processing Order

1. Delete `parsedOptions['--']` (cac-specific artifact)
2. Delete short-alias duplicate keys
3. Prototype pollution guard
4. Unknown option detection + warning
5. Snapshot `rawArgs` (includes unknown keys)
6. Remove unknown keys from `parsedOptions`
7. Type coercion — duplicate filtering + array wrapping (single merged loop)
8. Object option parsing (`key=val,key=val`)
9. `normalizeCliOptions()` — valibot validation + input/output splitting

## Implementation Notes

### `CACError` Is Not Exported

cac only exports `cac`, `CAC`, and `Command`. `CACError` is in `utils.ts` but not re-exported. We catch by checking `err.name === 'CACError'`.

### `ignoreOptionDefaultValue()`

cac auto-injects `default: true` for `--no-*` options. Without `ignoreOptionDefaultValue()`, cac injects these into every parse result, even when the flag is not passed. This breaks valibot validation — e.g. `preserveEntrySignatures` only accepts `false`, so cac's injected `true` causes a validation error. We disable cac's defaults entirely and let the bundler handle its own defaults.

### Short-Alias Key Duplication

mri returns both short and long names as separate keys (e.g. `-s` → `{ s: true, sourcemap: true }`). We collect all short aliases at startup and delete them from parsed options.

### Nested Option Parent Keys

cac's `setDotProp` converts `--transform.define value` into `{ transform: { define: 'value' } }`. When checking for unknown options, the top-level key `transform` is not in flattened `schemaInfo` (only `transform.define`, `transform.target`, etc. are). We pre-compute parent keys from dot-separated schema keys and include them in the known set.

### Object Option Parsing Traversal

After cac's `setDotProp`, parsed options already have nested structure. The object parsing step traverses the dot-path to find and parse string values, rather than iterating top-level entries.

### `--config` With Optional Value

`-c` registered as `[optional]` returns `config: true` when no value follows. `normalize.ts` maps `config: true` → `config: ''` to preserve auto-detect behavior.

### `--environment` Is Not an Object Option

`--environment` uses `:` and `,` separators (Rollup-compatible), processed separately in `cli/index.ts` by writing to `process.env`. Schema type is `string | string[]`, not object. Unrelated to the object option parsing.

### `--` Delimiter

`parseArgs` treats args after `--` as positionals. cac collects them into `options['--']` as an array. We delete this key in post-processing since no downstream code uses it.

## Edge Cases

### `--sourcemap` Dual Behavior

`-s` alone → `true`. `-s inline` → `"inline"`. `--sourcemap hidden` → `"hidden"`.

Registered as `-s, --sourcemap [type]`. The `[optional]` bracket means mri does NOT treat `-s` as boolean — it consumes the next non-flag arg as the value, or returns `true` if none follows.

### `--no-preserve-entry-signatures`

When passed, cac sets `preserveEntrySignatures: false`. When not passed, it's `undefined` and the bundler applies its own default (`ExportsOnly`).

### Object Options With Comma in Values

`--transform.define __A__=A,__B__=B` — cac returns the single string `"__A__=A,__B__=B"`. Our post-processing splits it into `{ __A__: 'A', __B__: 'B' }`.

### Prototype Pollution

cac's `setDotProp` does not guard against `__proto__`, `constructor`, or `prototype`. We delete any such keys in post-processing before normalization.

## Test Cases

Tests are in `packages/rolldown/tests/cli/cli-e2e.test.ts`. Run with `cd packages/rolldown/tests && pnpm test:cli`.

| #   | Feature                   | Example                                                                         |
| --- | ------------------------- | ------------------------------------------------------------------------------- |
| 1   | `--version` / `-v`        | `rolldown --version`                                                            |
| 2   | `--help` / `-h`           | `rolldown --help`                                                               |
| 3   | Help for empty args       | `rolldown`                                                                      |
| 4   | Help precedence ([#8523]) | `rolldown lib -o dist/lib.js --help`                                            |
| 5   | Boolean options           | `rolldown index.ts --minify -d dist`                                            |
| 6   | String options            | `rolldown index.ts --format cjs -d dist`                                        |
| 7   | Short flags               | `rolldown index.ts -d dist -s`                                                  |
| 8   | Array (repeated flags)    | `rolldown index.ts --external node:path --external node:url -d dist`            |
| 9   | Object (repeated flags)   | `rolldown index.ts --module-types .123=text --module-types .b64=base64 -d dist` |
| 9a  | Object (comma-separated)  | `rolldown index.ts --module-types .123=text,notjson=json,.b64=base64 -d dist`   |
| 10  | `--no-*` boolean negation | `rolldown index.ts --no-external-live-bindings ...`                             |
| 11  | Nested dot-notation       | `rolldown index.js --transform.define __DEFINE__=defined`                       |
| 12  | Positionals as input      | `rolldown 1.ts --input ./2.js`                                                  |
| 13  | Config loading (`-c`)     | `rolldown -c rolldown.config.ts`                                                |
| 14  | Config function + rawArgs | `rolldown -c rolldown.config.js --customArg=customValue`                        |
| 15  | CLI overrides config      | `rolldown -c rolldown.config.js --format cjs`                                   |
| 16  | `--environment`           | `rolldown -c --environment PRODUCTION,FOO:bar`                                  |
| 17  | `requireValue` validation | `rolldown 1.ts -d` (error: requires value)                                      |
| 18  | Invalid option value      | `rolldown index.ts --format INCORRECT`                                          |
| 19  | Unknown option warns      | `rolldown index.ts --someRandomFlag -d dist`                                    |
| 20  | Watch mode                | `rolldown index.ts -d dist -w -s`                                               |
| 21  | camelCase input ([#8410]) | `rolldown index.ts --moduleTypes .png=dataurl -d dist`                          |

[#8410]: https://github.com/rolldown/rolldown/issues/8410
[#3248]: https://github.com/rolldown/rolldown/issues/3248
[#8523]: https://github.com/rolldown/rolldown/issues/8523

## Related

- [#8410 — CLI silently mishandles camelCase options](https://github.com/rolldown/rolldown/issues/8410)
- [#3248 — `-s inline` position restriction](https://github.com/rolldown/rolldown/issues/3248)
- [#8523 — `--help` precedence over other options](https://github.com/rolldown/rolldown/issues/8523)
- [Vite CLI source](https://github.com/vitejs/vite/blob/main/packages/vite/src/node/cli.ts) — reference for cac usage patterns

---

<details>
<summary>Migration context (archived)</summary>

## Migration: `parseArgs` → cac

The previous implementation used Node.js's built-in `parseArgs` with 16 hand-rolled workarounds. The root cause of #8410 was that `parseArgs` treats `--moduleTypes` as an unknown boolean (since it only knows `--module-types`), silently dropping the value into positionals.

### What Changed

| File                         | Action       | Details                                         |
| ---------------------------- | ------------ | ----------------------------------------------- |
| `cli/arguments/index.ts`     | Rewrite      | Replace parseArgs with cac, add post-processing |
| `cli/arguments/normalize.ts` | Simplify     | Remove unflattening loop + prototype guard      |
| `cli/arguments/alias.ts`     | Simplify     | Remove `default` field (dead code)              |
| `cli/arguments/utils.ts`     | Simplify     | Remove `kebabCaseToCamelCase`                   |
| `cli/commands/help.ts`       | Minor update | Adjust to new `options` export shape            |
| `cli/index.ts`               | No change    | Same interface                                  |
| `cli/commands/bundle.ts`     | No change    | Same interface                                  |

### Behavioral Differences

| Change                      | Before                                             | After                                                |
| --------------------------- | -------------------------------------------------- | ---------------------------------------------------- |
| Numeric string coercion     | `--code-splitting.min-size 1000` → string `"1000"` | → number `1000` (mri coerces numeric-looking values) |
| `--no-*` on unknown options | warns "foo is unrecognized"                        | same warning, value is `false` instead of absent     |
| `--` delimiter              | args after `--` become positionals                 | collected into `options['--']`, deleted in post-proc |
| Short flag stacking         | not supported                                      | `-ms` = `--minify --sourcemap`                       |

### Why `default` Was Removed From `alias.ts`

The three `reverse: true` options (`treeshake`, `externalLiveBindings`, `preserveEntrySignatures`) had `default` values that were dead code on main — the token loop only used `default` for `string`/`union` types passed without a value, and these are all `boolean`/`reverse` options. With `ignoreOptionDefaultValue()`, cac never applies defaults either.

</details>
