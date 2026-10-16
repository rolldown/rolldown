import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { aggregateProfile, profileEvidence, profileRun } from '../lib/profile.mjs';
import { findChrome, launchBrowser } from '../lib/cdp.mjs';
import { startServer } from '../lib/serve.mjs';

const origin = 'http://localhost:3000';
const chunk = (source, mappings = 'AAAA') => ({
  code: 'function work() { return 42; }',
  map: { version: 3, sources: [source], names: [], mappings },
});
const profile = (frames) => ({
  origin,
  nodes: frames.map((frame, i) => ({
    id: i + 1,
    callFrame: { functionName: 'work', lineNumber: 0, columnNumber: 0, ...frame },
  })),
  samples: frames.map((_, i) => i + 1),
  timeDeltas: frames.map((frame) => frame.micros ?? 20_000),
});

test('CPU attribution maps all split chunks and aggregates a source across chunks', () => {
  const reads = [];
  const result = aggregateProfile(
    profile([
      { url: `${origin}/entry.js` },
      { url: `${origin}/chunks/%40app.js?version=1` },
      { url: `${origin}/chunks/@app.js?version=2` },
      { url: `${origin}/dynamic.js` },
    ]),
    {
      readChunk: (file) => {
        reads.push(file);
        return chunk(file === 'entry.js' ? 'src/entry.ts' : 'src/startup.ts');
      },
    },
  );
  assert.deepEqual(reads, ['entry.js', 'chunks/@app.js', 'dynamic.js']);
  assert.deepEqual(result.rows[0], {
    bucket: 'src/startup.ts',
    kind: 'source',
    ms: 60,
    chunks: ['chunks/@app.js', 'dynamic.js'],
  });
  assert.equal(result.totalMs, 80);
  assert.equal(result.attribution.sourceMs, 80);
  assert.equal(profileEvidence(result).state, 'open');
});

test('missing and malformed sourcemaps remain unknown instead of engine work', () => {
  const result = aggregateProfile(
    profile([
      { url: `${origin}/missing.js` },
      { url: `${origin}/bad.js` },
      { url: `${origin}/unmapped.js` },
      { url: `${origin}/position.js`, lineNumber: -1 },
    ]),
    {
      readChunk: (file) => {
        if (file === 'missing.js') return null;
        if (file === 'bad.js') throw new Error('bad map');
        return chunk('src/file.ts', file === 'unmapped.js' ? 'A' : 'AAAA');
      },
    },
  );
  assert.deepEqual(
    new Set(result.rows.map((r) => r.kind)),
    new Set(['no-sourcemap', 'invalid-sourcemap', 'unmapped']),
  );
  assert.equal(result.attribution.unattributedMs, 80);
  assert.equal(result.attribution.engineMs, 0);
  assert.equal(profileEvidence(result).state, 'unknown');
});

test('cross-origin lookalikes and encoded traversal never read local files', () => {
  const result = aggregateProfile(
    profile([
      { url: 'https://external.example/entry.js?secret=synthetic' },
      { url: `${origin}/%2e%2e%2fprivate.js` },
      { url: `${origin}/%5cprivate.js` },
      { url: `${origin}/%2fprivate.js` },
      { url: `${origin}/bad%escape.js` },
      { url: `${origin}/` },
    ]),
    { readChunk: () => assert.fail('must not read these paths') },
  );
  assert.equal(result.attribution.unattributedMs, 120);
  assert.equal(
    result.rows.find((r) => r.kind === 'external').bucket,
    'https://external.example/entry.js',
  );
});

test('a frame without a URL counts for its nearest caller with a URL, else for the engine', () => {
  const frames = [
    { id: 1, callFrame: { functionName: '(idle)', url: '' } },
    { id: 2, callFrame: { functionName: '(program)', url: '' } },
    { id: 3, callFrame: { functionName: '(garbage collector)', url: '' } },
    {
      id: 4,
      callFrame: {
        functionName: 'work',
        url: `${origin}/entry.js`,
        lineNumber: 0,
        columnNumber: 0,
      },
      children: [5],
    },
    { id: 5, callFrame: { functionName: 'now', url: '' } },
    { id: 6, callFrame: { functionName: '(anonymous)', url: '' } },
  ];
  const result = aggregateProfile(
    {
      origin,
      nodes: frames,
      samples: [1, 2, 3, 5, 6],
      timeDeltas: [1, 2, 3, 5, 6].map(() => 20_000),
    },
    { readChunk: () => chunk('src/work.ts') },
  );
  assert.equal(result.totalMs, 60);
  assert.deepEqual(result.attribution, { sourceMs: 20, engineMs: 40, unattributedMs: 0 });
  assert.deepEqual(
    result.rows.find((row) => row.kind === 'source'),
    { bucket: 'src/work.ts', kind: 'source', ms: 20, chunks: ['entry.js'] },
  );
  assert.equal(profileEvidence(result).state, 'open');
});

test('top-level code splits by sampled line and never names only the first module', () => {
  const code = 'const a = 1;\nconst b = 2;\nconst c = 3; const d = 4;';
  const map = {
    version: 3,
    sources: ['src/a.ts', 'src/b.ts'],
    names: [],
    mappings: 'AAAA;ACAA;ADAA,UCAA',
  };
  const topLevel = (positionTicks) => ({
    origin,
    nodes: [
      {
        id: 1,
        callFrame: { functionName: '', url: `${origin}/chunk.js`, lineNumber: 0, columnNumber: 0 },
        ...(positionTicks ? { positionTicks } : {}),
      },
    ],
    samples: [1],
    timeDeltas: [40_000],
  });
  const read = { readChunk: () => ({ code, map }) };
  const split = aggregateProfile(
    topLevel([
      { line: 1, ticks: 2 },
      { line: 2, ticks: 1 },
      { line: 3, ticks: 1 },
    ]),
    read,
  );
  assert.deepEqual(
    split.rows.map((row) => [row.bucket, row.kind, row.ms]),
    [
      ['src/a.ts', 'source', 20],
      ['chunk.js', 'top-level', 10],
      ['src/b.ts', 'source', 10],
    ],
  );
  const unsplit = aggregateProfile(topLevel(null), read);
  assert.deepEqual(
    unsplit.rows.map((row) => [row.bucket, row.kind, row.ms]),
    [['chunk.js', 'top-level', 40]],
  );
  assert.equal(profileEvidence(unsplit).state, 'unknown');
});

test('source candidates survive incomplete attribution and old reports are unknown', () => {
  const result = aggregateProfile(
    profile([{ url: `${origin}/chunk.js` }, { url: `${origin}/missing.js` }]),
    { readChunk: (file) => (file === 'chunk.js' ? chunk('single-file.ts') : null) },
  );
  const evidence = profileEvidence(result);
  assert.equal(evidence.state, 'open');
  assert.equal(evidence.candidates[0].bucket, 'single-file.ts');
  assert.equal(evidence.unattributed.length, 1);
  assert.equal(profileEvidence({ rows: [{ bucket: 'chunk.js', ms: 100 }] }).state, 'unknown');
});

test('clear means no source candidate, not proof that source work is required', () => {
  const result = aggregateProfile(
    profile([
      { url: `${origin}/small.js`, micros: 5_000 },
      { functionName: '(garbage collector)', url: '', micros: 100_000 },
    ]),
    { readChunk: () => chunk('src/small.ts') },
  );
  assert.equal(profileEvidence(result).state, 'clear');
  assert.equal(result.attribution.sourceMs, 5);
});

test('browser profile attributes work in a dynamic chunk to its source', async (t) => {
  try {
    findChrome();
  } catch {
    t.skip('Chrome/Edge is not installed');
    return;
  }
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'metrics-profile-'));
  const code =
    'export function work() { const end = performance.now() + 150; while (performance.now() < end) {} }';
  fs.writeFileSync(path.join(dir, 'work.js'), code);
  fs.writeFileSync(
    path.join(dir, 'index.html'),
    '<!doctype html><body><script type="module">const { work } = await import("/work.js"); work(); document.body.innerHTML = "<h1>Synthetic profile test</h1>";</script>',
  );
  const server = await startServer(dir);
  const browser = await launchBrowser({ profileDir: path.join(dir, 'chrome-profile') });
  t.after(async () => {
    await browser.close();
    await server.close();
    fs.rmSync(dir, { recursive: true, force: true });
  });
  const captured = await profileRun(browser.cdp, { origin: server.origin });
  assert.equal(captured.origin, server.origin);
  const result = aggregateProfile(captured, {
    readChunk: (file) => {
      assert.equal(file, 'work.js');
      return { ...chunk('src/expensive-work.ts'), code };
    },
  });
  assert.ok(
    result.rows.some(
      (r) => r.kind === 'source' && r.bucket === 'src/expensive-work.ts' && r.ms >= 15,
    ),
  );
});

test('empty profiles and unsupported maps do not produce a clear signal', () => {
  const empty = aggregateProfile(profile([]), { readChunk: () => null });
  assert.equal(profileEvidence(empty).state, 'unknown');
  const result = aggregateProfile(profile([{ url: `${origin}/broken.js` }]), {
    readChunk: () => ({ code: 'export {};', map: { version: 3, mappings: 'AAAA' } }),
  });
  assert.equal(result.rows[0].kind, 'invalid-sourcemap');
});
