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

- `--entry <file>` — file marked as a REPL entry. Repeat for multi-entry fixtures. Defaults to `entry.js` / `src/main.js` / `index.js` / `main.js` if found.
- `--version <v>` — rolldown version field (defaults to `latest`).
- `--base <url>` — base REPL URL (defaults to `https://repl.rolldown.rs/`).

The encoder skips `dist/`, `node_modules/`, `.git/`, Rolldown fixture harness files (`_config.json`, `_config.ts`, `_test.mjs`, `mod.rs`), and `.snap` files so integration-test fixtures can be encoded as-is. Build-time config from `_config.json` / `_config.ts` (e.g. `manualCodeSplitting`, `configVariants`) is not part of the REPL share format — the share URL carries only files + rolldown version, so the user must set those options in the REPL UI after loading the link.

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
3. Run the decoder with `--write /tmp/repl-<issue-number>/` to get the files.
4. Note the `v` field — if the issue is version-specific, install that exact version before reproducing.
5. Build/run rolldown against the decoded files and compare with the issue's described output.

**Producing a reproduction link (encode):**

1. Assemble or pick the source directory (e.g. an integration-test fixture).
2. Run the encoder with `--entry <file>` if the default detection won't pick the right one.
3. Round-trip through `decode.mjs` to sanity-check the file list and entry marker before sharing.
4. Paste the URL into the issue/PR.

## Notes

- The decoder handles both the modern zlib-compressed format (`\x78\xDA` header) and the legacy `decodeURIComponent(escape(...))` fallback that older share links used. The encoder always emits the modern format.
- If decoding fails with "Invalid base64", the URL was probably truncated when copied. Ask the user to repaste the full URL.
