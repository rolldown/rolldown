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

Every test case needed for the CLI. `[EXISTING]` = already in `packages/rolldown/tests/cli/cli-e2e.test.ts`. `[MISSING]` = needs to be added.

### Basic Arguments

| #   | Test                                                                                                     | Status       | Command                              | Expected                 |
| --- | -------------------------------------------------------------------------------------------------------- | ------------ | ------------------------------------ | ------------------------ |
| 1   | Help for empty args                                                                                      | `[EXISTING]` | `rolldown`                           | Exits 0, prints help     |
| 2   | No Node.js version warning                                                                               | `[EXISTING]` | `rolldown`                           | No "Please upgrade"      |
| 3   | `--version`                                                                                              | `[MISSING]`  | `rolldown --version`                 | Prints version, exits 0  |
| 4   | `-v`                                                                                                     | `[MISSING]`  | `rolldown -v`                        | Prints version, exits 0  |
| 5   | `--help`                                                                                                 | `[MISSING]`  | `rolldown --help`                    | Same as empty args help  |
| 6   | `-h`                                                                                                     | `[MISSING]`  | `rolldown -h`                        | Same as empty args help  |
| 6a  | `--help` takes precedence over other options ([#8523](https://github.com/rolldown/rolldown/issues/8523)) | `[MISSING]`  | `rolldown lib -o dist/lib.js --help` | Prints help (not bundle) |
| 6b  | `-h` takes precedence over other options                                                                 | `[MISSING]`  | `rolldown lib -o dist/lib.js -h`     | Prints help (not bundle) |

### Option Types

| #   | Test                       | Status       | Command                                                              | Expected            |
| --- | -------------------------- | ------------ | -------------------------------------------------------------------- | ------------------- |
| 7   | Boolean `--minify`         | `[EXISTING]` | `rolldown index.ts --minify -d dist`                                 | Minified output     |
| 8   | Short boolean `-m`         | `[EXISTING]` | `rolldown index.ts -m -d dist`                                       | Minified output     |
| 9   | String `--format cjs`      | `[EXISTING]` | `rolldown index.ts --format cjs -d dist`                             | CJS output          |
| 10  | Array repeated flags       | `[EXISTING]` | `rolldown index.ts --external node:path --external node:url -d dist` | Both externals      |
| 11  | Object `--module-types`    | `[EXISTING]` | `rolldown index.ts --module-types .123=text -d dist`                 | Module type applied |
| 12  | Object `--globals`         | `[MISSING]`  | `rolldown index.ts --format iife --globals jQuery=$ -d dist`         | Global mapping      |
| 13  | Union `--platform node`    | `[MISSING]`  | `rolldown index.ts --platform node -d dist`                          | Platform set        |
| 14  | Union `--sourcemap inline` | `[MISSING]`  | `rolldown index.ts --sourcemap inline -d dist`                       | Inline sourcemap    |
| 15  | Union `--sourcemap hidden` | `[MISSING]`  | `rolldown index.ts --sourcemap hidden -d dist`                       | Hidden sourcemap    |

### Short Flags

| #   | Test           | Status       | Command                                       | Expected            |
| --- | -------------- | ------------ | --------------------------------------------- | ------------------- |
| 16  | `-s` sourcemap | `[EXISTING]` | `rolldown index.ts -d dist -s`                | Sourcemap generated |
| 17  | `-f cjs`       | `[MISSING]`  | `rolldown index.ts -f cjs -d dist`            | CJS output          |
| 18  | `-e node:path` | `[MISSING]`  | `rolldown index.ts -e node:path -d dist`      | External applied    |
| 19  | `-n bundle`    | `[MISSING]`  | `rolldown index.ts -f iife -n bundle -d dist` | IIFE with name      |
| 20  | `-p node`      | `[MISSING]`  | `rolldown index.ts -p node -d dist`           | Platform set        |

### `--no-*` Boolean Negation

| #   | Test                             | Status       | Command                                                                          | Expected        |
| --- | -------------------------------- | ------------ | -------------------------------------------------------------------------------- | --------------- |
| 21  | `--no-external-live-bindings`    | `[EXISTING]` | `rolldown index.ts --format iife --external node:fs --no-external-live-bindings` | Disabled        |
| 22  | `--no-treeshake`                 | `[MISSING]`  | `rolldown index.ts --no-treeshake -d dist`                                       | Treeshaking off |
| 23  | `--no-preserve-entry-signatures` | `[MISSING]`  | `rolldown index.ts --no-preserve-entry-signatures -d dist`                       | Not preserved   |

### Nested Dot-Notation

| #   | Test                           | Status       | Command                                                                | Expected      |
| --- | ------------------------------ | ------------ | ---------------------------------------------------------------------- | ------------- |
| 24  | `--transform.define`           | `[EXISTING]` | `rolldown index.js --transform.define __DEFINE__=defined`              | Replaced      |
| 25  | Comma-separated define         | `[EXISTING]` | `rolldown index.js --transform.define __A__=A,__B__=B,__C__=C -d dist` | All replaced  |
| 26  | `--checks.circular-dependency` | `[MISSING]`  | `rolldown index.ts --checks.circular-dependency -d dist`               | Check enabled |
| 27  | `--transform.target`           | `[MISSING]`  | `rolldown index.ts --transform.target es2020 -d dist`                  | Target set    |

### Positionals and Input

| #   | Test                          | Status       | Command                                      | Expected          |
| --- | ----------------------------- | ------------ | -------------------------------------------- | ----------------- |
| 28  | `--input` + positional args   | `[EXISTING]` | `rolldown 1.ts --input ./2.js`               | Both bundled      |
| 29  | Multiple positionals          | `[MISSING]`  | `rolldown src/a.ts src/b.ts -d dist`         | Both bundled      |
| 30  | Positionals ignored with `-c` | `[MISSING]`  | `rolldown src/main.ts -c rolldown.config.js` | Config input used |

### Config

| #   | Test                          | Status       | Command                                                  | Expected              |
| --- | ----------------------------- | ------------ | -------------------------------------------------------- | --------------------- |
| 31  | `-c` with specific config     | `[EXISTING]` | `rolldown -c rolldown.config.js`                         | Uses specified config |
| 32  | `-c` without value            | `[EXISTING]` | `rolldown -c`                                            | Auto-detects config   |
| 33  | Config `.js` (CJS)            | `[EXISTING]` | `rolldown -c rolldown.config.js`                         | Loads                 |
| 34  | Config `.cjs`                 | `[EXISTING]` | `rolldown -c rolldown.config.cjs`                        | Loads                 |
| 35  | Config `.ts`                  | `[EXISTING]` | `rolldown -c rolldown.config.ts`                         | Loads                 |
| 36  | Config `.cts`                 | `[EXISTING]` | `rolldown -c rolldown.config.cts`                        | Loads                 |
| 37  | Config `.mts`                 | `[EXISTING]` | `rolldown -c rolldown.config.mts`                        | Loads                 |
| 38  | Config with tsx loader        | `[EXISTING]` | `rolldown -c rolldown.config.ts` (tsx)                   | Loads                 |
| 39  | Config with oxnode loader     | `[EXISTING]` | `rolldown -c rolldown.config.ts` (oxnode)                | Loads                 |
| 40  | Config from non-working dir   | `[EXISTING]` | `rolldown -c ./ext-ts/rolldown.config.ts`                | Loads                 |
| 41  | Multiple config options       | `[EXISTING]` | `rolldown -c rolldown.config.ts`                         | Multiple bundled      |
| 42  | Multiple outputs              | `[EXISTING]` | `rolldown -c rolldown.config.ts`                         | Multiple generated    |
| 43  | Custom CLI args passthrough   | `[EXISTING]` | `rolldown -c rolldown.config.js --customArg=customValue` | Received in config fn |
| 44  | Config exports null           | `[EXISTING]` | `rolldown -c rolldown.config.js`                         | Error                 |
| 45  | Config fn returns non-object  | `[EXISTING]` | `rolldown -c rolldown.config.ts`                         | Error                 |
| 46  | No config file found          | `[EXISTING]` | `rolldown -c` (no file)                                  | Error                 |
| 47  | CLI overrides config          | `[MISSING]`  | `rolldown -c rolldown.config.js --format cjs`            | CJS wins              |
| 48  | Options + outputOptions hooks | `[EXISTING]` | `rolldown -c rolldown.config.ts`                         | Hooks called          |

### Environment

| #   | Test                      | Status       | Command                                           | Expected        |
| --- | ------------------------- | ------------ | ------------------------------------------------- | --------------- |
| 49  | `--environment` key:value | `[EXISTING]` | `rolldown -c --environment PRODUCTION,FOO:bar`    | process.env set |
| 50  | Multiple `--environment`  | `[MISSING]`  | `rolldown -c --environment A:1 --environment B:2` | Both set        |

### Validation and Errors

| #   | Test                        | Status       | Command                                | Expected              |
| --- | --------------------------- | ------------ | -------------------------------------- | --------------------- |
| 51  | Invalid format value        | `[EXISTING]` | `rolldown index.ts --format INCORRECT` | Error                 |
| 52  | `-d` without value          | `[EXISTING]` | `rolldown 1.ts -d`                     | Error: requires value |
| 53  | `--dir` without value       | `[EXISTING]` | `rolldown 1.ts --dir`                  | Error: requires value |
| 54  | `-d .` explicit current dir | `[EXISTING]` | `rolldown 1.ts -d .`                   | OK                    |
| 55  | `-o` without value          | `[EXISTING]` | `rolldown 1.ts -o`                     | Error: requires value |
| 56  | `--file` without value      | `[EXISTING]` | `rolldown 1.ts --file`                 | Error: requires value |

### camelCase Input (Bug #8410)

| #   | Test                           | Status      | Command                                                       | Expected            |
| --- | ------------------------------ | ----------- | ------------------------------------------------------------- | ------------------- |
| 57  | `--moduleTypes` (camelCase)    | `[MISSING]` | `rolldown index.ts --moduleTypes .png=dataurl -d dist`        | Works (not ignored) |
| 58  | `--assetFileNames` (camelCase) | `[MISSING]` | `rolldown index.ts --assetFileNames [name][extname] -d dist`  | Applied             |
| 59  | `--chunkFileNames` (camelCase) | `[MISSING]` | `rolldown index.ts --chunkFileNames [name]-[hash].js -d dist` | Applied             |

### Unrecognized Options

| #   | Test                           | Status      | Command                                      | Expected                 |
| --- | ------------------------------ | ----------- | -------------------------------------------- | ------------------------ |
| 60  | Unknown option warns, no error | `[MISSING]` | `rolldown index.ts --someRandomFlag -d dist` | Warning, bundle succeeds |
| 61  | Unknown option in rawArgs      | `[MISSING]` | `rolldown -c --myCustom=value`               | Config fn receives it    |
| 62  | Multiple unknown options       | `[MISSING]` | `rolldown index.ts --foo --bar -d dist`      | Warning lists both       |

### Edge Cases

| #   | Test                                | Status      | Command                                                 | Expected           |
| --- | ----------------------------------- | ----------- | ------------------------------------------------------- | ------------------ |
| 63  | `--` delimiter                      | `[MISSING]` | `rolldown index.ts -d dist -- --not-an-option`          | Not parsed as flag |
| 64  | Prototype pollution `--__proto__`   | `[MISSING]` | `rolldown index.ts --__proto__.polluted true -d dist`   | Blocked            |
| 65  | Prototype pollution `--constructor` | `[MISSING]` | `rolldown index.ts --constructor.polluted true -d dist` | Blocked            |

### Watch Mode

| #   | Test                      | Status       | Command                                                   | Expected        |
| --- | ------------------------- | ------------ | --------------------------------------------------------- | --------------- |
| 66  | closeBundle hook          | `[EXISTING]` | `rolldown -c`                                             | Hook called     |
| 67  | Watch with output options | `[EXISTING]` | `rolldown index.ts -d dist -w -s`                         | dist/ + .map    |
| 68  | Watch multiple options    | `[EXISTING]` | `rolldown -c rolldown.config.ts -d watch-dist-options -w` | Both outputs    |
| 69  | Watch multiple outputs    | `[EXISTING]` | `rolldown -c rolldown.config.ts -d watch-dist-output -w`  | Both outputs    |
| 70  | Watch + options hooks     | `[EXISTING]` | `rolldown -c`                                             | Hooks called    |
| 71  | ROLLDOWN_WATCH=false      | `[EXISTING]` | `rolldown -c`                                             | watchMode false |
| 72  | ROLLDOWN_WATCH=true       | `[EXISTING]` | `rolldown -w -c`                                          | watchMode true  |

### Default Options

| #   | Test                   | Status       | Command                                      | Expected         |
| --- | ---------------------- | ------------ | -------------------------------------------- | ---------------- |
| 73  | Default config options | `[EXISTING]` | `rolldown -c`                                | Defaults applied |
| 74  | `--cwd` option         | `[MISSING]`  | `rolldown index.ts --cwd /some/path -d dist` | CWD changed      |

### Test Summary

- **Total**: 76
- **Existing**: 43
- **Missing**: 33
- **High-risk missing** (add before migration): #57-59 (camelCase), #60-62 (unrecognized options), #47 (CLI overrides config), #6a-6b (`--help` precedence, [#8523](https://github.com/rolldown/rolldown/issues/8523))

## Unresolved Questions

- Should we keep `key=val,key=val` object syntax or migrate to cac's dot-notation (`--module-types.png dataurl`)? Keeping current syntax avoids breaking changes but requires ~20 lines of custom parsing.
- Should unrecognized options remain as warnings, or should we switch to cac's default behavior (error)? Current behavior is warn + include in rawArgs for config function passthrough.
- Should help text use cac's built-in format or preserve the current custom layout (sorted by short flag, with examples and notes sections)?
- Is cac's `setDotProp` vulnerable to prototype pollution? If so, keep the existing guard.

## Related

- [#8410 — CLI silently mishandles camelCase options](https://github.com/rolldown/rolldown/issues/8410)
- [#8408 — Closed PR that attempted narrow camelCase fix](https://github.com/rolldown/rolldown/pull/8408)
- [Vite CLI source](https://github.com/vitejs/vite/blob/main/packages/vite/src/node/cli.ts) — reference for cac usage patterns
