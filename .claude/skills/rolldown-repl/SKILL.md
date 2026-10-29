---
name: rolldown-repl
description: Use to encode or decode rolldown REPL share URLs — `repl.rolldown.rs/#…`, `rolldown-repl.netlify.app/#…`, or any URL whose hash is a base64-encoded rolldown REPL payload. Decode whenever such a URL appears in the conversation (commonly in GitHub issues with a "minimal reproduction" link) to read the actual source files + rolldown version. Encode when you need to share a local set of source files as a REPL URL (e.g. to attach a live reproduction to an issue or PR). Trigger even if the user only pastes the URL without explicitly asking.
---

# Encode and decode rolldown REPL share URLs

The rolldown REPL stores all state (input files + selected rolldown version) in `location.hash` as `base64(zlib(JSON))`. Browsers never send the hash to the server, so naive HTTP fetches return an empty shell. This skill encodes and decodes that hash locally so you can read what the user had in their REPL, or produce a new URL from a directory of source files.

## When to use

**Decode:**

- An issue or message links to `https://repl.rolldown.rs/#…` (or any other host serving the rolldown REPL).
- The user pastes such a URL and asks something like "what's wrong with this", "why does this fail", or "reproduce this bug".
- You're investigating a rolldown bug and the reporter included a REPL link as the reproduction.

If the URL has no `#…` payload, there's nothing to decode — tell the user and stop.

**Encode:**

- You have a local directory of source files (e.g. an integration-test fixture) and need to produce a shareable REPL URL — typically to attach a live reproduction to a GitHub issue or PR.
- The user asks to "create a REPL link" or "share this reproduction in the REPL".

## How

Run the bundled scripts. They use plain Node (`node:zlib`, `node:buffer`, `node:fs`) — no `npm install` needed.

> **Temp files: write outside the repo, and clean up after.** Never write decoded files, round-trip checks, or captured URLs into the project directory — they show up as untracked clutter in the user's working tree. Always target a path under the OS temp dir (POSIX: `/tmp/...`; Windows PowerShell: `$env:TEMP\...`), and **delete any temp file or directory you created once you're done** (e.g. `Remove-Item -Recurse -Force "$env:TEMP\repl-roundtrip"`). Do not pipe the URL to a `.txt` file inside the repo to "save" it — print it to the chat instead.

**Decode** prints a JSON summary to stdout, and with `--write` drops each file onto disk:

```bash
node .claude/skills/rolldown-repl/scripts/decode.mjs '<full-url>'
node .claude/skills/rolldown-repl/scripts/decode.mjs '<full-url>' --write /tmp/repl-repro
```

Quote the URL — the `#` would otherwise be eaten by the shell.

**Encode** walks a directory and prints the resulting REPL URL to stdout:

```bash
node .claude/skills/rolldown-repl/scripts/encode.mjs <dir>
node .claude/skills/rolldown-repl/scripts/encode.mjs <dir> --entry entry.js --version 1.0.0-rc.18
node .claude/skills/rolldown-repl/scripts/encode.mjs <dir> --entry a.js --entry b.js
```

Options:

- `--entry <file>` — file marked as a REPL entry. Repeat for multi-entry fixtures. Overrides entries auto-detected from `_config.json`. Defaults to `entry.js` / `src/main.js` / `index.js` / `main.js` / `index.ts` if found.
- `--version <v>` — rolldown version field (defaults to `latest`).
- `--base <url>` — base REPL URL (defaults to `https://repl.rolldown.rs/`).
- `--variant <name>` — when `_config.json` has `configVariants`, merge the named variant over the base config before generating `rolldown.config.ts`. Without it, only the base config is encoded (a note lists the skipped variants).
- `--no-config` — skip `_config.json` translation entirely and emit source files only.

The encoder skips `dist/`, `node_modules/`, `.git/`, Rolldown fixture harness files (`_config.ts`, `_test.mjs`, `mod.rs`), and `.snap` files so integration-test fixtures can be encoded as-is.

### `_config.json` → `rolldown.config.ts`

The REPL share state is only `{ files, version }` — there is no separate "options" field. But the REPL stores the bundle config as an ordinary **file** in the map: a `rolldown.config.ts` that does `input: import.meta.input`. So the encoder translates a fixture's sibling `_config.json` into exactly that file and embeds it in the payload, instead of silently dropping the config. Concretely it:

- Reads `_config.json`'s `config` (a flattened Rust `BundlerOptions`) and emits a `defineConfig({...})`, **nesting output options under `output`** (e.g. `codeSplitting`, `format`, `entryFileNames`), nesting `define`/`dropLabels` under `transform` and `profilerNames` under `output.generatedCode`, and applying the `Filename`→`FileNames` renames. Input options (`platform`, `external`, `resolve`, `experimental`, …) stay top level.
- **Auto-marks the entries** declared in `config.input` (so you usually don't need `--entry`), preserving named entries as `input: { name: 'file' }` and using `import.meta.input` for unnamed input.
- Prints `note:`/`warning:` lines to **stderr** for anything it couldn't translate cleanly: plugins (can't be serialized — port manually), unknown options (left top-level, verify), and skipped `configVariants`. The URL itself is the only thing on stdout, so `$(encode.mjs …)` still captures just the URL.

A `_config.ts` (TS, not JSON) can't be parsed — the encoder warns and emits files only; port its options into the REPL by hand. The translation table covers the options fixtures commonly use; if rolldown's option surface changes, the table in `encode.mjs` (`OUTPUT_FIELDS` / `OUTPUT_RENAMES` / `NESTED_FIELDS`) may need updating — always sanity-check the generated `rolldown.config.ts` after decoding.

## Payload shape

```json
{
  "v": "<rolldown version, e.g. 'latest' or '1.2.3'>",
  "f": {
    "src/index.js": { "n": "src/index.js", "c": "...source...", "e": true },
    "...": { ... }
  }
}
```

Where `n` is the filename, `c` is the source content, and `e` (when present) marks the entry file. The decoder reshapes this into a summary (file list + sizes + version) for quick reading; use `--write <dir>` when you actually need the source on disk.

## Typical workflows

**Issue investigation (decode):**

1. Read the GitHub issue (e.g. `gh issue view 9211 -R rolldown/rolldown`).
2. Spot the REPL URL in the body.
3. Run the decoder with `--write` to a temp dir **outside the repo** (e.g. `/tmp/repl-<issue-number>/` or `$env:TEMP\repl-<issue-number>`) to get the files.
4. Note the `v` field — if the issue is version-specific, install that exact version before reproducing.
5. Build/run rolldown against the decoded files and compare with the issue's described output.
6. Delete the temp dir when you're finished.

**Producing a reproduction link (encode):**

1. Assemble or pick the source directory (e.g. an integration-test fixture).
2. Run the encoder. If the fixture has a `_config.json`, entries and build options come from it automatically; pass `--entry <file>` only to override, and `--variant <name>` to pick a config variant.
3. Round-trip through `decode.mjs --write <dir>` (a temp dir **outside the repo**) and read the generated `rolldown.config.ts` to confirm the options translated correctly before sharing.
4. Paste the URL into the issue/PR, then **delete the temp dir** you used for the round-trip.

## Notes

- The decoder handles both the modern zlib-compressed format (`\x78\xDA` header) and the legacy `decodeURIComponent(escape(...))` fallback that older share links used. The encoder always emits the modern format.
- If decoding fails with "Invalid base64", the URL was probably truncated when copied. Ask the user to repaste the full URL.
- Before finishing, double-check the working tree (`git status`) for any `.tmp*` / round-trip leftovers the scripts produced and remove them — the only lasting output of this skill should be the URL (or the files the user explicitly asked you to write).
