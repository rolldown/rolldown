#!/usr/bin/env node
// Iterate the corpus, spawn one probe per file with builtin's transform config,
// detect oxc-side-effects-unreachable panics via stderr scan, kill on detection.

import { spawn } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const CORPUS_JSON = join(__dirname, 'corpus.json');
const SKIP_JSON = join(__dirname, 'builtin-skip.json');
const PROBE = join(__dirname, '_probe-one.mjs');

const corpus = JSON.parse(readFileSync(CORPUS_JSON, 'utf8'));
const start = Number(process.env.START ?? 0);
const limit = Number(process.env.LIMIT ?? corpus.files.length);
const files = corpus.files.slice(start, start + limit);
const ROOT = corpus.root;
const TIMEOUT_MS = Number(process.env.PROBE_TIMEOUT_MS ?? 5000);

function probeOne(absPath) {
  return new Promise((resolve) => {
    const proc = spawn('node', [PROBE, absPath], { stdio: ['ignore', 'pipe', 'pipe'] });
    let outcome = null;
    const finish = (r) => {
      if (outcome) return;
      outcome = r;
      try { proc.kill('SIGKILL'); } catch {}
      resolve(r);
    };

    const onChunk = (buf) => {
      const s = buf.toString();
      if (/unreachable code|panicked at/.test(s)) finish('PANIC');
    };
    proc.stderr.on('data', onChunk);
    proc.stdout.on('data', (buf) => {
      const s = buf.toString();
      if (s.includes('OK')) finish('OK');
      else if (s.includes('PANIC')) finish('PANIC');
      else if (s.includes('ERROR')) finish('ERROR');
    });
    proc.on('exit', (code) => {
      if (!outcome) finish(code === 0 ? 'OK' : 'ERROR');
    });
    setTimeout(() => finish('TIMEOUT'), TIMEOUT_MS);
  });
}

// Merge with existing skip list so partial runs accumulate.
let prev = { panicked: [], errored: [], timeouts: [] };
try {
  const fs = await import('node:fs');
  prev = JSON.parse(fs.readFileSync(SKIP_JSON, 'utf8'));
} catch {}
const panicked = [...(prev.panicked ?? [])];
const errored = [...(prev.errored ?? [])];
const timeouts = [...(prev.timeouts ?? [])];
const seen = new Set([...panicked, ...errored, ...timeouts]);

function add(arr, file) {
  if (!seen.has(file)) {
    arr.push(file);
    seen.add(file);
  }
}

console.log(`Probing files[${start}..${start + files.length}] (${files.length} files), timeout=${TIMEOUT_MS}ms`);
for (let i = 0; i < files.length; i++) {
  const file = files[i];
  const r = await probeOne(join(ROOT, file));
  let mark = '.';
  if (r === 'PANIC') { add(panicked, file); mark = '!'; }
  else if (r === 'TIMEOUT') { add(timeouts, file); mark = 'T'; }
  else if (r === 'ERROR') { add(errored, file); mark = 'x'; }
  process.stdout.write(mark);
  if ((i + 1) % 50 === 0) process.stdout.write(` ${start + i + 1}\n`);
}
process.stdout.write('\n');

console.log(`panicked: ${panicked.length}, errored: ${errored.length}, timeouts: ${timeouts.length}`);
writeFileSync(SKIP_JSON, JSON.stringify({ panicked, errored, timeouts }, null, 2) + '\n');
console.log(`wrote ${SKIP_JSON}`);
