# CLI Refactor: Replace `parseArgs` with `cac`

## Summary

The current CLI argument parsing (`packages/rolldown/src/cli/arguments/`) uses Node.js's built-in `parseArgs` with ~330 lines of custom workarounds. We replace it with [cac](https://github.com/cacjs/cac) (v6.7.14), the same CLI library used by Vite and tsdown. This fixes the camelCase option bug ([#8410](https://github.com/rolldown/rolldown/issues/8410)) and eliminates most of the custom parsing code.

## CLI Features Currently in Use

### Option Types

| Type                               | Example                                                       | Current Implementation                                       |
| ---------------------------------- | ------------------------------------------------------------- | ------------------------------------------------------------ |
| boolean                            | `--minify`, `--watch`                                         | `parseArgs` native + manual `--no-*` handling                |
| string                             | `--dir dist`, `--format cjs`                                  | `parseArgs` native                                           |
| object (`key=val,key=val`)         | `--module-types .png=dataurl,.svg=text`, `--globals jQuery=$` | Manual split on `,` then `=` (`arguments/index.ts:109-129`)  |
| array (repeated flags)             | `--external jquery --external lodash`                         | Manual push into array (`arguments/index.ts:130-139`)        |
| union (string with limited values) | `--format esm`, `--platform node`, `--sourcemap inline`       | Falls through to default case (`arguments/index.ts:147-156`) |

### Short Flags

| Short | Long          | Type                    |
| ----- | ------------- | ----------------------- |
| `-c`  | `--config`    | string (optional value) |
| `-h`  | `--help`      | boolean                 |
| `-v`  | `--version`   | boolean                 |
| `-w`  | `--watch`     | boolean                 |
| `-d`  | `--dir`       | string (requireValue)   |
| `-o`  | `--file`      | string (requireValue)   |
| `-e`  | `--external`  | array                   |
| `-f`  | `--format`    | union                   |
| `-n`  | `--name`      | string                  |
| `-g`  | `--globals`   | object                  |
| `-s`  | `--sourcemap` | union (default: `true`) |
| `-m`  | `--minify`    | boolean                 |
| `-p`  | `--platform`  | union                   |

### `--no-*` Boolean Negation

Three options use `reverse: true` in `alias.ts`:

| CLI Flag                         | Sets                              | Default    |
| -------------------------------- | --------------------------------- | ---------- |
| `--no-external-live-bindings`    | `externalLiveBindings = false`    | `true`     |
| `--no-treeshake`                 | `treeshake = false`               | `true`     |
| `--no-preserve-entry-signatures` | `preserveEntrySignatures = false` | `"strict"` |

### Nested Dot-Notation Options

Options with `.` in the key get unflattened into nested objects (`normalize.ts:28-37`):

```bash
--transform.define __A__=A,__B__=B
--transform.target es2020
--transform.drop-labels debugOnly
--checks.circular-dependency
--optimization.inline-const
--devtools.session-id xxx
--code-splitting.min-size 1000
```

### Other Features

- **Positionals as input files**: `rolldown src/main.ts src/other.ts` — positionals become `input.input`, only when `--config` is not set.
- **`--config` without value**: `rolldown -c` auto-detects config file. Enabled by `strict: false`.
- **`rawArgs` passthrough**: All parsed args (including unrecognized) are collected into `rawArgs` and passed to config functions (`export default (cliArgs) => ({ ... })`).
- **`--environment` processing**: `--environment KEY:VALUE,OTHER:VAL` — splits on `,` then `:`, sets `process.env`.
- **`requireValue` validation**: `--dir` and `--file` must have a value. Without this, `-d` alone silently targets current directory.
- **Unrecognized option warning**: Unknown options warn but do not error. They are included in `rawArgs`.
- **Prototype pollution guard**: `__proto__`, `constructor`, `prototype` keys are silently dropped.
- **Input/Output splitting**: Parsed options are split into `input` vs `output` based on schema keys.
- **Help text generation**: Auto-generated from the `options` object with sorting, padding, and hints.

## Hacks Built on Top of `parseArgs`

16 workarounds exist in the current implementation:

| #   | Hack                                                            | Goes Away with cac?                                           |
| --- | --------------------------------------------------------------- | ------------------------------------------------------------- |
| 1   | `strict: false` to allow unknown options and `-c` without value | Yes — cac has `allowUnknownOptions()` and `[optional]` syntax |
| 2   | Manual kebab-case → camelCase after parsing                     | Yes — cac converts automatically                              |
| 3   | Manual camelCase → kebab-case during registration               | Yes — cac accepts either                                      |
| 4   | `--no-*` prefix stripping and boolean inversion                 | Yes — cac handles natively                                    |
| 5   | Object parsing (`key=val,key=val` split)                        | **No** — cac has no record/map parsing                        |
| 6   | Array accumulation via repeated flags                           | Yes — cac auto-accumulates                                    |
| 7   | Union type handling with defaults                               | Yes — cac treats them as strings                              |
| 8   | String-type missing value with default injection                | Yes — cac `[optional]` syntax                                 |
| 9   | `requireValue` validation                                       | Yes — cac `<required>` syntax                                 |
| 10  | `Object.defineProperty` everywhere                              | Yes — not needed with cac                                     |
| 11  | Nested option unflattening                                      | Yes — cac has `setDotProp` built-in                           |
| 12  | Prototype pollution guard                                       | **Needs custom code** — cac's `setDotProp` is not safe        |
| 13  | Input/Output option splitting                                   | **No** — rolldown-specific logic                              |
| 14  | Invalid option collection + warning                             | **Partially** — cac allows unknown options but doesn't warn   |
| 15  | `rawArgs` assembly                                              | **Partially** — need to capture from cac's parsed result      |
| 16  | Reverse option description rewriting for help                   | Yes — cac's `--no-*` handles help text                        |

10 hacks go away entirely. 2 still need custom code. 4 are partially handled.

## Behavioral Differences Between cac and Current CLI

### Unrecognized options: warn vs error

Current `parseArgs` **warns** on unknown flags and includes them in `rawArgs`. cac **throws `CACError`** by default.

Workaround: Use `.allowUnknownOptions()` to preserve the "warn but don't error" behavior, then manually compare parsed options against known schema keys to print a warning.

### camelCase input

Current `parseArgs` **silently ignores** `--moduleTypes .png=dataurl` (treated as unknown boolean, value lost to positionals). cac **works correctly** — it converts kebab-case and camelCase interchangeably. This is the bug that prompted the migration.

### Repeated flags for non-array options

Current behavior: last value wins for non-array types (e.g. `--format cjs --format esm` → `esm`). cac auto-accumulates into arrays (→ `["cjs", "esm"]`).

Workaround: Same as Vite's `filterDuplicateOptions()` — take last value for non-array option types.

### Object parsing (`key=val,key=val`)

Current behavior: `--module-types .png=dataurl,.svg=text` is manually split into `{ ".png": "dataurl", ".svg": "text" }`. cac treats this as a plain string.

Options:

1. Keep manual `key=val,key=val` parsing (~20 lines) to preserve current syntax (recommended)
2. Change to dot-notation: `--module-types.png dataurl` (breaking change)

### Nested dot-notation

Behavior is identical. cac's `setDotProp` does the same as rolldown's `setNestedProperty`. The `camelcaseOptionName` function in cac only camelCases the first segment before the dot, matching current behavior.

### `--no-*` negation

Registration syntax changes (cac uses `.option('--no-treeshake', '...')`) but runtime behavior is identical. cac auto-sets default to `true` when `--no-*` is defined.

### Help text generation

Current help is fully custom (sorted by short flag, padded columns, hints, examples, notes). cac auto-generates a different format via `cli.help()`. cac supports a help callback for customization.

### Prototype pollution

cac's `setDotProp` does **not** guard against `--__proto__`. Must keep the prototype pollution check.

## Features Only in Rolldown CLI (Not in Vite)

These are features rolldown uses through CLI that Vite only exposes through config files:

| Feature                                  | Notes                                                                 |
| ---------------------------------------- | --------------------------------------------------------------------- |
| Object parsing (`key=val,key=val`)       | Vite only uses objects in config files                                |
| `--no-*` boolean negation                | Vite has zero `--no-*` options                                        |
| Nested dot-notation options              | Vite's nesting is config-file-only                                    |
| Array accumulation                       | Vite deduplicates repeated flags (takes last value)                   |
| `rawArgs` passthrough to config function | Vite's config function receives `{ mode, command }`, not raw CLI args |
| `--environment` → `process.env`          | Vite uses `--mode` instead                                            |

### Vite's cac hacks (for reference)

1. **Duplicate option filtering** — takes last value when repeated (`filterDuplicateOptions`)
2. **Global option cleaning** — strips global flags before passing to command config
3. **Custom type converters** — `--host 0` and `--base 0` (numeric → string)
4. **Sourcemap string→boolean coercion** — `"true"` → `true`
5. **Watch flag→object conversion** — `true` → `{}`
6. **Early debug processing** — before cac loads

## Test Cases

Tests grouped by **CLI feature**. Each feature needs at least one test to verify its behavior before and after the cac migration.

`[EXISTING]` = covered in `packages/rolldown/tests/cli/cli-e2e.test.ts`. `[MISSING]` = needs to be added.

| #   | Feature                         | Status       | Example                                                              | Notes                                                                                       |
| --- | ------------------------------- | ------------ | -------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| 1   | `--version` / `-v`              | `[EXISTING]` | `rolldown --version`                                                 |                                                                                             |
| 2   | `--help` / `-h`                 | `[EXISTING]` | `rolldown --help`                                                    |                                                                                             |
| 3   | Help for empty args             | `[EXISTING]` | `rolldown`                                                           |                                                                                             |
| 4   | Help precedence over other opts | `[MISSING]`  | `rolldown lib -o dist/lib.js --help`                                 | [#8523](https://github.com/rolldown/rolldown/issues/8523)                                   |
| 5   | Boolean options                 | `[EXISTING]` | `rolldown index.ts --minify -d dist`                                 | Also tests short flag `-m`                                                                  |
| 6   | String options                  | `[EXISTING]` | `rolldown index.ts --format cjs -d dist`                             |                                                                                             |
| 7   | Short flags                     | `[EXISTING]` | `rolldown index.ts -d dist -s`                                       | `-d`, `-s`, `-m`, `-c` etc. already covered across tests                                    |
| 8   | Array (repeated flags)          | `[EXISTING]` | `rolldown index.ts --external node:path --external node:url -d dist` |                                                                                             |
| 9   | Object (`key=val,key=val`)      | `[EXISTING]` | `rolldown index.ts --module-types .123=text -d dist`                 | `--globals` not separately tested but same code path                                        |
| 10  | `--no-*` boolean negation       | `[EXISTING]` | `rolldown index.ts --no-external-live-bindings ...`                  | One of three `--no-*` options tested                                                        |
| 11  | Nested dot-notation             | `[EXISTING]` | `rolldown index.js --transform.define __DEFINE__=defined`            | Tests single and comma-separated values                                                     |
| 12  | Positionals as input            | `[EXISTING]` | `rolldown 1.ts --input ./2.js`                                       |                                                                                             |
| 13  | Config loading (`-c`)           | `[EXISTING]` | `rolldown -c rolldown.config.ts`                                     | Tests `.js`, `.cjs`, `.ts`, `.cts`, `.mts`, auto-detect, multiple configs, multiple outputs |
| 14  | Config function + rawArgs       | `[EXISTING]` | `rolldown -c rolldown.config.js --customArg=customValue`             | Unknown options passed to config function                                                   |
| 15  | CLI overrides config            | `[EXISTING]` | `rolldown -c rolldown.config.js --format cjs`                        |                                                                                             |
| 16  | `--environment`                 | `[EXISTING]` | `rolldown -c --environment PRODUCTION,FOO:bar`                       |                                                                                             |
| 17  | `requireValue` validation       | `[EXISTING]` | `rolldown 1.ts -d` / `rolldown 1.ts -o`                              | `-d`, `--dir`, `-o`, `--file` without value → error                                         |
| 18  | Invalid option value            | `[EXISTING]` | `rolldown index.ts --format INCORRECT`                               |                                                                                             |
| 19  | Unknown option warns (no error) | `[EXISTING]` | `rolldown index.ts --someRandomFlag -d dist`                         |                                                                                             |
| 20  | Watch mode                      | `[EXISTING]` | `rolldown index.ts -d dist -w -s`                                    | Tests `-w`, watch hooks, multiple configs, `ROLLDOWN_WATCH` env                             |
| 21  | camelCase input ([#8410])       | `[MISSING]`  | `rolldown index.ts --moduleTypes .png=dataurl -d dist`               | The bug that prompted the migration — camelCase options are silently ignored                |

[#8410]: https://github.com/rolldown/rolldown/issues/8410

### Summary

- **Existing**: 20 features covered
- **Missing**: 1 feature to add before migration
  - camelCase input (#21) — the core bug

CLI tests: `cd packages/rolldown/tests && pnpm test:cli`

## Unresolved Questions

- Should we keep `key=val,key=val` object syntax or migrate to cac's dot-notation (`--module-types.png dataurl`)? Keeping current syntax avoids breaking changes but requires ~20 lines of custom parsing.
- Should unrecognized options remain as warnings, or should we switch to cac's default behavior (error)? Current behavior is warn + include in rawArgs for config function passthrough.
- Should help text use cac's built-in format or preserve the current custom layout (sorted by short flag, with examples and notes sections)?
- Is cac's `setDotProp` vulnerable to prototype pollution? If so, keep the existing guard.

## Related

- [#8410 — CLI silently mishandles camelCase options](https://github.com/rolldown/rolldown/issues/8410)
- [#8408 — Closed PR that attempted narrow camelCase fix](https://github.com/rolldown/rolldown/pull/8408)
- [Vite CLI source](https://github.com/vitejs/vite/blob/main/packages/vite/src/node/cli.ts) — reference for cac usage patterns
