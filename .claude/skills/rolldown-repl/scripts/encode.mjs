#!/usr/bin/env node
// Encode a set of source files into a rolldown REPL share URL.
//
// Inverse of decode.mjs. Builds the hash payload as
// base64(zlib.deflate(JSON({v: version, f: { filename: {n, c, e?} }}))).
//
// Usage:
//   node encode.mjs <dir> [--entry <file>]... [--version <v>] [--base <repl-url>]
//
// All non-dotfile files under <dir> are included. Use --entry to mark a file
// as a REPL entry; repeat the flag for multi-entry fixtures (e.g.
// `--entry a.js --entry b.js`). Defaults to entry.js / src/main.js /
// index.js / main.js if found.

import { argv, exit } from 'node:process';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative, sep } from 'node:path';
import { Buffer } from 'node:buffer';
import zlib from 'node:zlib';

const SKIP_DIRS = new Set(['dist', 'node_modules', '.git']);
const SKIP_FILES = new Set(['_config.json', '_config.ts', '_test.mjs', 'mod.rs']);
const SKIP_EXT = ['.snap'];

function walk(dir, base = dir, out = []) {
  for (const name of readdirSync(dir)) {
    if (name.startsWith('.')) continue;
    const full = join(dir, name);
    const st = statSync(full);
    if (st.isDirectory()) {
      if (SKIP_DIRS.has(name)) continue;
      walk(full, base, out);
    } else {
      if (SKIP_FILES.has(name)) continue;
      if (SKIP_EXT.some((ext) => name.endsWith(ext))) continue;
      out.push(relative(base, full).split(sep).join('/'));
    }
  }
  return out;
}

function main() {
  const args = argv.slice(2);
  let dir = null;
  const entries = [];
  let version = 'latest';
  let base = 'https://repl.rolldown.rs/';
  for (let i = 0; i < args.length; i++) {
    const a = args[i];
    if (a === '--entry') entries.push(args[++i]);
    else if (a === '--version') version = args[++i];
    else if (a === '--base') base = args[++i];
    else if (!dir) dir = a;
  }
  if (!dir) {
    console.error('usage: encode.mjs <dir> [--entry <file>]... [--version <v>] [--base <url>]');
    exit(2);
  }

  const files = walk(dir);
  if (entries.length === 0) {
    for (const cand of ['entry.js', 'src/main.js', 'index.js', 'main.js']) {
      if (files.includes(cand)) {
        entries.push(cand);
        break;
      }
    }
  }
  const entrySet = new Set(entries);

  const f = {};
  for (const name of files) {
    const c = readFileSync(join(dir, name), 'utf-8');
    f[name] = entrySet.has(name) ? { n: name, c, e: true } : { n: name, c };
  }

  const json = JSON.stringify({ v: version, f });
  const compressed = zlib.deflateSync(Buffer.from(json, 'utf-8'), { level: 9 });
  const b64 = compressed.toString('base64');
  const url = base.replace(/#.*$/, '').replace(/\/?$/, '/') + '#' + b64;
  console.log(url);
}

main();
