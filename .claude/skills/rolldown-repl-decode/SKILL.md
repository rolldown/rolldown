---
name: rolldown-repl-decode
description: Use whenever a rolldown REPL share URL appears in the conversation — `repl.rolldown.rs/#…`, `rolldown-repl.netlify.app/#…`, or any URL whose hash is a base64-encoded rolldown REPL payload. This commonly happens when investigating GitHub issues in rolldown/rolldown that include a "minimal reproduction" REPL link. Decodes the hash into the actual source files + rolldown version so the issue can be reproduced or understood. Trigger even if the user only pastes the URL without explicitly asking to decode it.
---

# Decode a rolldown REPL share URL

The rolldown REPL stores all state (input files + selected rolldown version) in `location.hash` as `base64(zlib(JSON))`. Browsers never send the hash to the server, so naive HTTP fetches return an empty shell. This skill decodes the hash locally so you can read what the user actually had in their REPL.

## When to use

- An issue or message links to `https://repl.rolldown.rs/#…` (or any other host serving the rolldown REPL).
- The user pastes such a URL and asks something like "what's wrong with this", "why does this fail", or "reproduce this bug".
- You're investigating a rolldown bug and the reporter included a REPL link as the reproduction.

If the URL has no `#…` payload, there's nothing to decode — tell the user and stop.

## How

Run the bundled decoder. It prints a JSON summary to stdout, and with `--write` it also drops each file onto disk:

```bash
node .claude/skills/rolldown-repl-decode/scripts/decode.mjs '<full-url>'
node .claude/skills/rolldown-repl-decode/scripts/decode.mjs '<full-url>' --write /tmp/repl-repro
```

Quote the URL — the `#` would otherwise be eaten by the shell.

## What you get

The decoded JSON has shape:

```json
{
  "v": "<rolldown version, e.g. 'latest' or '1.2.3'>",
  "f": {
    "src/index.js": { "n": "src/index.js", "c": "...source...", "e": true },
    "...": { ... }
  }
}
```

Where `n` is the filename, `c` is the source content, and `e` (when present) marks the entry file. The script reshapes this into a summary (file list + sizes + version) for quick reading. Use `--write <dir>` when you actually need the source on disk to reproduce — e.g. to run `pnpm rolldown` against it.

## Typical workflow for issue investigation

1. Read the GitHub issue (e.g. `gh issue view 9211 -R rolldown/rolldown`).
2. Spot the REPL URL in the body.
3. Run the decoder with `--write /tmp/repl-<issue-number>/` to get the files.
4. Note the `v` field — if the issue is version-specific, install that exact version before reproducing.
5. Build/run rolldown against the decoded files and compare with the issue's described output.

## Notes

- The decoder handles both the modern zlib-compressed format (`\x78\xDA` header) and the legacy `decodeURIComponent(escape(...))` fallback that older share links used.
- Plain Node `.mjs` (uses `node:zlib`, `node:buffer`, `node:fs`). No `npm install` needed.
- If decoding fails with "Invalid base64", the URL was probably truncated when copied. Ask the user to repaste the full URL.
