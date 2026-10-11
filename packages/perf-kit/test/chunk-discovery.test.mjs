import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { analyzeChunkDiscovery, loadChunkGraph } from '../lib/chunk-discovery.mjs';
import { postLoadScripts } from '../lib/measure.mjs';

const analyzeLcpDiscovery = (...args) => analyzeChunkDiscovery(...args).lcp;

const graph = {
  file: 'chunk-graph.json',
  chunks: new Map([
    ['assets/facade.js', { imports: ['assets/shared.js'] }],
    ['assets/shared.js', { imports: ['assets/app.js', 'assets/facade.js'] }],
    ['assets/app.js', { imports: [] }],
    ['assets/optional.js', { imports: [] }],
  ]),
};
const script = (file, start, end, bytes = 10) => ({
  name: `/assets/${file}.js`,
  type: 'script',
  start,
  end,
  bytes,
});
const sample = (resources, rest = {}) => ({
  load: 400,
  fcp: 1000,
  lcp: 1500,
  resourceDetailsComplete: true,
  resources,
  ...rest,
});

test('a tiny facade exposes a late transitive static graph, with cycles excluded', () => {
  const result = analyzeLcpDiscovery(
    graph,
    [
      sample([
        script('facade', 100, 200, 1),
        script('shared', 700, 800, 1000),
        script('app', 710, 900, 2000),
        script('optional', 720, 950, 5000),
      ]),
    ],
    150,
  );
  assert.equal(result.state, 'open');
  assert.equal(result.groups.length, 1);
  const group = result.groups[0];
  assert.equal(group.parent, 'assets/facade.js');
  assert.equal(group.bytes, 3000);
  assert.equal(group.count, 2);
  assert.equal(group.maxDelayMs, 510);
  assert.deepEqual(
    group.example.chunks.map((r) => r.file),
    ['assets/app.js', 'assets/shared.js'],
  );
});

test('preloaded descendants, unfinished scripts and scripts after LCP do not open a lead', () => {
  const result = analyzeLcpDiscovery(
    graph,
    [
      sample([
        script('facade', 100, 200),
        script('shared', 110, 300),
        script('app', 700, 0),
        script('optional', 900, 1800),
      ]),
    ],
    150,
  );
  assert.equal(result.state, 'clear');
  assert.deepEqual(result.groups, []);
});

test('an RTT boundary is strict; cross-origin names do not match local chunks', () => {
  const result = analyzeLcpDiscovery(
    graph,
    [
      sample([
        script('facade', 100, 200),
        script('shared', 350, 800),
        { ...script('app', 700, 900), name: 'https://example.invalid/assets/app.js' },
      ]),
    ],
    150,
  );
  assert.equal(result.state, 'clear');
  assert.equal(result.matchedScripts, 2);
});

test('escaped pathname characters match the original chunk filenames', () => {
  for (const file of ['shared#.js', 'shared?.js', 'shared%.js', 'shared space.js', '共享.js']) {
    const escapedGraph = {
      file: 'chunk-graph.json',
      chunks: new Map([
        ['assets/facade.js', { imports: [`assets/${file}`] }],
        [`assets/${file}`, { imports: [] }],
      ]),
    };
    const result = analyzeLcpDiscovery(
      escapedGraph,
      [
        sample([
          script('facade', 100, 200),
          { ...script('shared', 700, 800), name: `/assets/${encodeURIComponent(file)}?v=1#hash` },
        ]),
      ],
      150,
    );
    assert.equal(result.state, 'open', file);
    assert.equal(result.matchedScripts, 2);
    assert.equal(result.groups[0].example.chunks[0].file, `assets/${file}`);
  }
});

test('missing graphs, buffer overflow, missing LCP and unmatched scripts stay UNKNOWN', () => {
  assert.equal(analyzeLcpDiscovery(null, [], 150).state, 'unknown');
  for (const rest of [{ resourceDetailsComplete: false }, { lcp: null }, { resources: [] }]) {
    const result = analyzeLcpDiscovery(graph, [sample([script('facade', 100, 200)], rest)], 150);
    assert.equal(result.state, 'unknown');
  }
  assert.equal(
    analyzeLcpDiscovery(
      graph,
      [sample([script('facade', 100, 200)]), sample([script('facade', 100, 200)]), sample([])],
      150,
    ).state,
    'unknown',
  );
  assert.equal(analyzeLcpDiscovery(graph, [sample([])], 0).state, 'unknown');
});

test('a lead records occurrence counts without adding overlapping graph bytes', () => {
  const late = sample([script('facade', 100, 200), script('shared', 700, 800)]);
  const early = sample([script('facade', 100, 200), script('shared', 110, 300)]);
  const result = analyzeLcpDiscovery(graph, [late, early, late], 150);
  assert.equal(result.groups[0].runsAffected, 2);
  assert.equal(result.groups[0].measuredRuns, 3);
});

test('FCP and LCP use separate paint windows and retain actionable import chains', () => {
  const result = analyzeChunkDiscovery(
    graph,
    [sample([script('facade', 100, 200), script('shared', 700, 800), script('app', 1100, 1200)])],
    150,
  );
  assert.equal(result.fcp.state, 'open');
  assert.equal(result.lcp.state, 'open');
  const early = result.fcp.groups.find((g) => g.parent === 'assets/facade.js');
  const late = result.lcp.groups.find((g) => g.parent === 'assets/facade.js');
  assert.deepEqual(
    early.example.chunks.map((c) => c.file),
    ['assets/shared.js'],
  );
  assert.equal(early.example.paintMs, 1000);
  assert.equal(late.example.paintMs, 1500);
  assert.equal(late.paintDependency, 'unverified');
  assert.deepEqual(late.example.chunks.find((c) => c.file === 'assets/app.js').importChain, [
    'assets/facade.js',
    'assets/shared.js',
    'assets/app.js',
  ]);
});

test('missing one paint does not discard evidence for the other', () => {
  const resources = [script('facade', 100, 200), script('shared', 700, 800)];
  const noLcp = analyzeChunkDiscovery(graph, [sample(resources, { lcp: null })], 150);
  assert.equal(noLcp.fcp.state, 'open');
  assert.equal(noLcp.lcp.state, 'unknown');
  const noFcp = analyzeChunkDiscovery(graph, [sample(resources, { fcp: null })], 150);
  assert.equal(noFcp.fcp.state, 'unknown');
  assert.equal(noFcp.lcp.state, 'open');
  const afterFcp = analyzeChunkDiscovery(graph, [sample(resources, { fcp: 600 })], 150);
  assert.equal(afterFcp.fcp.state, 'clear');
  assert.equal(afterFcp.lcp.state, 'open');
});

test('post-load download evidence separates FCP from LCP and excludes unfinished requests', () => {
  const result = postLoadScripts([
    sample([
      script('before-load', 390, 500, 100),
      script('first', 450, 600, 200),
      script('later', 1100, 1400, 400),
      script('unfinished', 500, 0, 800),
      script('after', 1600, 1700, 1600),
    ]),
  ]);
  assert.deepEqual(result.fcp, { count: 1, bytes: 200, firstStartMs: 450, lastEndMs: 600 });
  assert.deepEqual(result.lcp, { count: 2, bytes: 600, firstStartMs: 450, lastEndMs: 1400 });
});

test('a Vite manifest joins import keys to filenames and excludes dynamic imports', (t) => {
  const dist = fs.mkdtempSync(path.join(os.tmpdir(), 'chunk-discovery-'));
  t.after(() => fs.rmSync(dist, { recursive: true, force: true }));
  fs.mkdirSync(path.join(dist, '.vite'));
  fs.mkdirSync(path.join(dist, 'assets'));
  for (const name of ['facade', 'shared', 'app', 'optional'])
    fs.writeFileSync(path.join(dist, `assets/${name}.js`), '');
  fs.writeFileSync(
    path.join(dist, '.vite/manifest.json'),
    JSON.stringify({
      facade: { file: 'assets/facade.js', imports: ['shared'], dynamicImports: ['optional'] },
      shared: { file: 'assets/shared.js', imports: ['app'] },
      app: { file: 'assets/app.js' },
      optional: { file: 'assets/optional.js' },
    }),
  );
  const loaded = loadChunkGraph(dist);
  assert.deepEqual(loaded.chunks.get('assets/facade.js').imports, ['assets/shared.js']);
  assert.deepEqual(loaded.chunks.get('assets/shared.js').imports, ['assets/app.js']);
  const sidecar = path.join(dist, 'chunk-graph.json');
  fs.writeFileSync(
    sidecar,
    JSON.stringify({
      schemaVersion: 1,
      chunks: [
        {
          fileName: 'dist/assets/facade.js',
          facadeModuleId: 'src/main.ts',
          imports: ['dist/assets/shared.js'],
        },
        { fileName: 'dist/assets/shared.js', imports: [] },
      ],
    }),
  );
  assert.equal(loadChunkGraph(dist, [sidecar]).file, sidecar);
  assert.equal(
    loadChunkGraph(dist, [sidecar]).chunks.get('assets/facade.js').facadeModuleId,
    'src/main.ts',
  );
  fs.writeFileSync(
    sidecar,
    JSON.stringify({ schemaVersion: 1, chunks: [{ fileName: 'dist/assets/stale.js' }] }),
  );
  assert.equal(loadChunkGraph(dist, [sidecar]).file, loaded.file);
});

test('a newer manifest replaces an old graph when a rebuild reuses filenames', (t) => {
  const dist = fs.mkdtempSync(path.join(os.tmpdir(), 'chunk-discovery-'));
  t.after(() => fs.rmSync(dist, { recursive: true, force: true }));
  fs.mkdirSync(path.join(dist, '.vite'));
  fs.mkdirSync(path.join(dist, 'assets'));
  const before = new Date('2026-01-01T00:00:00Z');
  const after = new Date('2026-01-01T00:00:10Z');
  for (const name of ['facade', 'shared']) {
    const file = path.join(dist, `assets/${name}.js`);
    fs.writeFileSync(file, '');
    fs.utimesSync(file, before, before);
  }
  const sidecar = path.join(dist, 'chunk-graph.json');
  fs.writeFileSync(
    sidecar,
    JSON.stringify({
      schemaVersion: 1,
      chunks: [
        { fileName: 'assets/facade.js', imports: [] },
        { fileName: 'assets/shared.js', imports: [] },
      ],
    }),
  );
  fs.utimesSync(sidecar, before, before);
  const manifest = path.join(dist, '.vite/manifest.json');
  fs.writeFileSync(
    manifest,
    JSON.stringify({
      facade: { file: 'assets/facade.js', imports: ['shared'] },
      shared: { file: 'assets/shared.js' },
    }),
  );
  fs.utimesSync(manifest, after, after);

  const loaded = loadChunkGraph(dist, [sidecar]);
  assert.equal(loaded.file, manifest);
  assert.equal(
    analyzeLcpDiscovery(
      loaded,
      [sample([script('facade', 100, 200), script('shared', 700, 800)])],
      150,
    ).state,
    'open',
  );
});

test('a rewritten chunk invalidates an old graph without a manifest', (t) => {
  const dist = fs.mkdtempSync(path.join(os.tmpdir(), 'chunk-discovery-'));
  t.after(() => fs.rmSync(dist, { recursive: true, force: true }));
  const sidecar = path.join(dist, 'chunk-graph.json');
  fs.writeFileSync(
    sidecar,
    JSON.stringify({ schemaVersion: 1, chunks: [{ fileName: 'facade.js', imports: [] }] }),
  );
  const chunk = path.join(dist, 'facade.js');
  fs.writeFileSync(chunk, '');
  const before = new Date('2026-01-01T00:00:00Z');
  const after = new Date('2026-01-01T00:00:10Z');
  fs.utimesSync(sidecar, before, before);
  fs.utimesSync(chunk, after, after);
  assert.equal(loadChunkGraph(dist, [sidecar]), null);
  fs.mkdirSync(path.join(dist, '.vite'));
  const manifest = path.join(dist, '.vite/manifest.json');
  fs.writeFileSync(manifest, JSON.stringify({ facade: { file: 'facade.js' } }));
  fs.utimesSync(manifest, before, before);
  assert.equal(loadChunkGraph(dist, [sidecar]), null);
  fs.utimesSync(manifest, after, after);
  assert.equal(loadChunkGraph(dist, [sidecar]).file, manifest);
  fs.utimesSync(sidecar, after, after);
  assert.equal(loadChunkGraph(dist, [sidecar]).file, sidecar);
});
