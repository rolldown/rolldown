// Server-startup mode. The parts worth pinning are the ones a wrong answer makes
// SILENT: a readiness spec that parses into the wrong probe measures the wrong moment,
// an inherited env var inflates every run, and coverage attribution that drops a script
// makes a heavy module look absent rather than cold.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

import { parseReadySpec, describeReady, runEnv } from '../lib/node/launch.mjs';
import {
  attributeStartupCoverage,
  coldModules,
  graphIdCandidates,
  splitProfileOwnership,
  summarizeStartup,
} from '../lib/node/measure.mjs';
import { makeGraph } from './graph-fixtures.mjs';
import { resolveModule } from '../lib/module-graph.mjs';

// --- readiness specs --------------------------------------------------------------

test('every supported --ready form parses to its own probe kind', () => {
  assert.deepEqual(parseReadySpec('exit'), { kind: 'exit' });
  assert.deepEqual(parseReadySpec(''), { kind: 'exit' });
  assert.deepEqual(parseReadySpec('port:3000'), { kind: 'port', port: 3000 });
  assert.equal(parseReadySpec('http://127.0.0.1:3000/health').kind, 'http');
  const stdout = parseReadySpec('stdout:listening on \\d+');
  assert.equal(stdout.kind, 'stdout');
  assert.ok(stdout.pattern.test('server listening on 3000'));
  assert.ok(!stdout.pattern.test('still booting'));
});

test('an unparseable ready spec names every supported form', () => {
  assert.throws(
    () => parseReadySpec('whenever'),
    (err) => {
      // The message IS the fix: an agent that reads only the error still learns the menu.
      for (const form of ['port:', 'stdout:', 'http', 'exit'])
        assert.match(err.message, new RegExp(form));
      return true;
    },
  );
  assert.throws(() => parseReadySpec('port:99999'), /not a valid port/);
  assert.throws(() => parseReadySpec('stdout:[unclosed'), /not a valid regex/);
});

test('describeReady round-trips into something a report can be compared on', () => {
  assert.equal(describeReady(parseReadySpec('port:8080')), 'port 8080 accepting');
  assert.equal(describeReady(parseReadySpec('exit')), 'process exits 0');
});

// --- run environment --------------------------------------------------------------

test('inherited coverage and inspector settings never reach the measured process', () => {
  const env = runEnv({
    cacheMode: 'cold',
    cacheDir: '/tmp/cache-1',
    extraEnv: {
      NODE_V8_COVERAGE: '/tmp/cov',
      NODE_OPTIONS: '--inspect=9229 --max-old-space-size=4096',
    },
  });
  // Coverage instrumentation would inflate every run; a second inspector would steal
  // the port our own attach scrapes.
  assert.equal(env.NODE_V8_COVERAGE, undefined);
  assert.equal(env.NODE_OPTIONS, '--max-old-space-size=4096');
  assert.equal(env.NODE_COMPILE_CACHE, '/tmp/cache-1');
});

test('NODE_OPTIONS disappears when it held nothing but inspector flags', () => {
  const env = runEnv({ cacheMode: 'off', extraEnv: { NODE_OPTIONS: '--inspect-brk' } });
  assert.equal(env.NODE_OPTIONS, undefined);
});

test('cache mode decides who owns NODE_COMPILE_CACHE', () => {
  assert.equal(runEnv({ cacheMode: 'cold', cacheDir: '/tmp/a' }).NODE_COMPILE_CACHE, '/tmp/a');
  assert.equal(runEnv({ cacheMode: 'warm', cacheDir: '/tmp/b' }).NODE_COMPILE_CACHE, '/tmp/b');
  // off = leave the ambient environment alone, even if the caller passed a dir.
  assert.equal(
    runEnv({ cacheMode: 'off', cacheDir: '/tmp/c', extraEnv: { NODE_COMPILE_CACHE: '/tmp/x' } })
      .NODE_COMPILE_CACHE,
    undefined,
  );
});

// --- summarize --------------------------------------------------------------------

test('app startup is total minus the runtime boot floor, never negative', () => {
  const summary = summarizeStartup([{ startupMs: 300 }, { startupMs: 340 }, { startupMs: 320 }], {
    bootFloorMs: 30,
    ready: 'port 3000 accepting',
  });
  assert.equal(summary.metrics['runtime.startup_ms'], 320);
  assert.equal(summary.metrics['runtime.boot_floor_ms'], 30);
  assert.equal(summary.metrics['runtime.app_startup_ms'], 290);

  // A target that comes up faster than the measured floor (floor sampled under load)
  // must report 0 addressable time, not a negative budget.
  const fast = summarizeStartup([{ startupMs: 25 }], { bootFloorMs: 30 });
  assert.equal(fast.metrics['runtime.app_startup_ms'], 0);
});

test('run-to-run spread is reported so untrustworthy deltas can be refused', () => {
  const steady = summarizeStartup([{ startupMs: 100 }, { startupMs: 102 }, { startupMs: 101 }], {});
  assert.ok(steady.guard.spreadPct < 5);
  const noisy = summarizeStartup([{ startupMs: 100 }, { startupMs: 400 }, { startupMs: 120 }], {});
  assert.ok(noisy.guard.spreadPct > 25);
  assert.equal(noisy.guard.allRunsCompleted, true);
});

// --- coverage attribution ---------------------------------------------------------

// Two source modules in one generated chunk. Line 0 maps to src/a.ts, line 2 to
// src/b.ts — mappings are written literally rather than VLQ-encoded in the test so the
// fixture stays readable: "AAAA" = all-zero deltas, "ACAA" = source index +1.
const CHUNK_CODE = ['const A = 1;', '// filler', 'const B = 2;'].join('\n');
const CHUNK_MAP = {
  version: 3,
  sources: ['src/a.ts', 'src/b.ts'],
  mappings: 'AAAA;;ACAA',
};

function writeChunk(dir) {
  fs.mkdirSync(dir, { recursive: true });
  const file = path.join(dir, 'server.js');
  fs.writeFileSync(file, CHUNK_CODE);
  fs.writeFileSync(`${file}.map`, JSON.stringify(CHUNK_MAP));
  return file;
}

function tmpRoot(tag) {
  return fs.mkdtempSync(path.join(os.tmpdir(), `metrics-lab-${tag}-`));
}

test('executed-at-ready bytes attribute to the source module that ran', (t) => {
  const root = tmpRoot('cov');
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const file = writeChunk(root);
  const bStart = CHUNK_CODE.indexOf('const B');

  const result = attributeStartupCoverage(
    [
      {
        url: pathToFileURL(file).href,
        functions: [
          // Top level ran to the end of module a; module b's region never executed.
          {
            functionName: '',
            ranges: [{ startOffset: 0, endOffset: CHUNK_CODE.length, count: 1 }],
          },
          {
            functionName: 'b',
            ranges: [{ startOffset: bStart, endOffset: CHUNK_CODE.length, count: 0 }],
          },
        ],
      },
    ],
    { root },
  );

  const byName = Object.fromEntries(result.modules.map((m) => [m.source, m]));
  assert.equal(byName['src/a.ts'].readyBytes, byName['src/a.ts'].totalBytes);
  assert.equal(byName['src/a.ts'].readyRatio, 1);
  assert.equal(byName['src/b.ts'].readyBytes, 0);
  assert.equal(byName['src/b.ts'].readyRatio, 0);
});

test('scripts outside the build are counted, not silently dropped', (t) => {
  const root = tmpRoot('external');
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  writeChunk(root);
  const outside = tmpRoot('outside');
  t.after(() => fs.rmSync(outside, { recursive: true, force: true }));
  const strayFile = path.join(outside, 'native-dep.js');
  fs.writeFileSync(strayFile, 'x');

  const result = attributeStartupCoverage(
    [
      // Node internals are runtime cost, not startup weight the bundler chose.
      { url: 'node:internal/modules/esm/loader', functions: [] },
      { url: pathToFileURL(strayFile).href, functions: [] },
    ],
    { root },
  );

  assert.equal(result.modules.length, 0);
  // The stray script counts; node: internals do not — otherwise every report would
  // claim dozens of unattributed scripts and the number would mean nothing.
  assert.equal(result.externalScriptCount, 1);
});

test('a local script without a sourcemap is reported, never counted as zero-weight', (t) => {
  const root = tmpRoot('nomap');
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const file = path.join(root, 'server.js');
  fs.writeFileSync(file, CHUNK_CODE);

  const result = attributeStartupCoverage([{ url: pathToFileURL(file).href, functions: [] }], {
    root,
  });
  // No map means localScript rejects it: it lands in the external count rather than
  // contributing 0 bytes to the module table and looking like dead weight.
  assert.equal(result.modules.length, 0);
  assert.equal(result.externalScriptCount, 1);
});

// --- leads ------------------------------------------------------------------------

test('cold modules are the big ones that never ran, ordered by what they cost', () => {
  const cold = coldModules([
    { source: 'src/heavy-unused.ts', totalBytes: 40_000, readyRatio: 0 },
    { source: 'src/small-unused.ts', totalBytes: 500, readyRatio: 0 },
    { source: 'src/hot.ts', totalBytes: 90_000, readyRatio: 0.9 },
    { source: 'src/medium-unused.ts', totalBytes: 9_000, readyRatio: 0.01 },
  ]);
  assert.deepEqual(
    cold.map((m) => m.source),
    ['src/heavy-unused.ts', 'src/medium-unused.ts'],
  );
});

test('a sourcemap source resolves to the graph id it corresponds to', () => {
  // The regression this pins: coverage names the module relative to the OUTPUT dir, the
  // graph names it relative to the PROJECT. Matching only the literal string silently
  // priced every cold module as "no match" and the scan stopped suggesting deferrals.
  assert.deepEqual(graphIdCandidates('../src/reporting.js'), [
    '../src/reporting.js',
    'src/reporting.js',
  ]);
  assert.deepEqual(graphIdCandidates('../../packages/app/src/x.ts'), [
    '../../packages/app/src/x.ts',
    '../packages/app/src/x.ts',
    'packages/app/src/x.ts',
  ]);
  assert.deepEqual(graphIdCandidates('src/plain.ts'), ['src/plain.ts']);
  assert.deepEqual(graphIdCandidates('..\\src\\win.ts'), ['../src/win.ts', 'src/win.ts']);

  // ...and the candidate list actually finds the module in a real graph.
  const graph = makeGraph(
    [
      { id: 'src/server.js', bytes: 10, imports: [[1, false]] },
      { id: 'src/reporting.js', bytes: 900, imports: [] },
    ],
    ['src/server.js'],
  );
  const hit = graphIdCandidates('../src/reporting.js')
    .map((form) => resolveModule(graph, form))
    .find((r) => r && !r.ambiguous);
  assert.equal(graph.modules[hit.index].id, 'src/reporting.js');
});

test('a measurement only a few probe intervals wide says so', () => {
  const tight = summarizeStartup([{ startupMs: 33 }], { bootFloorMs: 30, resolutionMs: 1 });
  // 3ms of addressable time at 1ms resolution: real, but not resolvable in ms.
  assert.deepEqual(tight.guard.resolutionLimited, { appMs: 3, resolutionMs: 1 });

  const roomy = summarizeStartup([{ startupMs: 330 }], { bootFloorMs: 30, resolutionMs: 1 });
  assert.equal(roomy.guard.resolutionLimited, null);
  // Event-observed probes (stdout/exit) do not quantize at all.
  assert.equal(
    summarizeStartup([{ startupMs: 33 }], { bootFloorMs: 30 }).guard.resolutionLimited,
    null,
  );
});

test('profile ownership separates what the bundle owns from what it never will', () => {
  const rows = [
    { bucket: 'src/app.ts', ms: 40 },
    { bucket: 'src/db.ts', ms: 10 },
    { bucket: '(engine: gc)', ms: 5 },
    { bucket: 'binding.node', ms: 245 },
  ];
  const split = splitProfileOwnership(rows, ['src/app.ts', 'src/db.ts']);
  assert.equal(split.appMs, 50);
  assert.equal(split.engineMs, 5);
  assert.equal(split.runtimeMs, 245);
  // 25% is the threshold the scan uses to say "the bundle is not your problem";
  // 50 of 300ms must land below it.
  assert.equal(split.appShare, 16.7);
});
