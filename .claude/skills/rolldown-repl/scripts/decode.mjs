#!/usr/bin/env node
// Decode a rolldown REPL share URL into its source files + version.
//
// The REPL encodes state as base64(zlib(JSON({f: files, v: version}))) in the
// URL hash. Implementation matches rolldown/repl `app/utils/url.ts` (utoa/atou).
//
// Usage:
//   node decode.mjs '<full-url-or-just-the-hash>'
//   node decode.mjs '<url>' --write <out-dir>   # also write files to disk

import { argv, exit } from 'node:process';
import { writeFileSync, mkdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { Buffer } from 'node:buffer';
import zlib from 'node:zlib';

function decodeHash(raw) {
  if (raw.includes('#')) raw = raw.split('#').slice(1).join('#');
  raw = decodeURIComponent(raw);
  // Pad base64 if needed.
  raw += '='.repeat(-raw.length & 3);
  const bin = Buffer.from(raw, 'base64');
  let text;
  if (bin[0] === 0x78 && bin[1] === 0xda) {
    // Modern format: zlib-compressed.
    text = zlib.inflateSync(bin).toString('utf-8');
  } else {
    // Legacy format: decodeURIComponent(escape(binary))
    text = decodeURIComponent(bin.toString('latin1'));
  }
  return JSON.parse(text);
}

function fileBytes(meta) {
  // The actual REPL payload uses `c` for content and `n` for filename.
  // Older payloads may use `code` — accept both.
  return (meta?.c ?? meta?.code ?? '').length;
}

function fileContent(meta) {
  return meta?.c ?? meta?.code ?? '';
}

function main() {
  const args = argv.slice(2);
  const writeIdx = args.indexOf('--write');
  const writeDir = writeIdx >= 0 ? args[writeIdx + 1] : null;
  const positional = args.filter((a, i) => a !== '--write' && (writeIdx < 0 || i !== writeIdx + 1));
  const url = positional[0];
  if (!url) {
    console.error('usage: decode.mjs <url> [--write <dir>]');
    exit(2);
  }

  const state = decodeHash(url);
  const files = state.f || {};
  const version = state.v || 'unknown';

  const summary = {
    version,
    file_count: Object.keys(files).length,
    files: Object.entries(files).map(([name, meta]) => ({
      filename: name,
      language: meta?.language ?? null,
      bytes: fileBytes(meta),
    })),
  };
  console.log(JSON.stringify(summary, null, 2));

  if (writeDir) {
    mkdirSync(writeDir, { recursive: true });
    for (const [name, meta] of Object.entries(files)) {
      const path = join(writeDir, name);
      mkdirSync(dirname(path), { recursive: true });
      writeFileSync(path, fileContent(meta), 'utf-8');
    }
    console.error(`\nwrote ${Object.keys(files).length} files to ${writeDir}`);
  }
}

main();
