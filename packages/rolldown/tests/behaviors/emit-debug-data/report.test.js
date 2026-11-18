import { rolldown } from 'rolldown';
import { defineParallelPlugin } from 'rolldown/experimental';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { availableParallelism, tmpdir } from 'node:os';
import { join } from 'node:path';
import { expect, test } from 'vitest';
// This folder has its own `package.json`, so `rolldown-tests/utils` does not resolve here.
import { isWasiTest } from '../../src/utils';

// A client and an SSR build of one app used to overwrite one report folder. Each output
// folder now has its own report under `node_modules/.rolldown/report/<output dir>`.
test('report mode writes one report folder per output folder', async () => {
  const cwd = mkdtempSync(join(tmpdir(), 'rolldown-report-dir-'));
  const sources = { client: 'console.log("client");', server: 'export const render = () => 1;' };
  try {
    for (const [name, dir] of [
      ['client', 'dist/client'],
      ['server', 'dist/server'],
      ['client', undefined],
    ]) {
      const bundle = await rolldown({
        cwd,
        input: { [name]: name },
        devtools: { mode: 'report' },
        plugins: [
          {
            name: 'virtual',
            resolveId: (id) => (id in sources ? id : null),
            load: (id) => sources[id],
          },
        ],
      });
      await bundle.write({ dir });
      await bundle.close();
    }
    const report = (dir) => join(cwd, 'node_modules/.rolldown/report', dir, 'metrics.json');
    const chunks = (dir) =>
      JSON.parse(readFileSync(report(dir), 'utf8')).chunks.largest.map((chunk) => chunk.file);
    expect(chunks('dist/client')).toEqual(['client.js']);
    expect(chunks('dist/server')).toEqual(['server.js']);
    // Without `dir`, the output folder is Rolldown's default `dist`.
    expect(chunks('dist')).toEqual(['client.js']);
    expect(existsSync(join(cwd, 'node_modules/.rolldown/report/metrics.json'))).toBe(false);
  } finally {
    rmSync(cwd, { recursive: true, force: true });
  }
});

// A no-op call returned nothing or the code unchanged. A call that the hook filter skips never
// runs the handler, so the report leaves it out.
test('report mode counts no-op calls and leaves out calls that the filter skips', async () => {
  const cwd = mkdtempSync(join(tmpdir(), 'rolldown-report-noop-'));
  const sources = { entry: 'import "dep"; console.log(1);', dep: 'console.log(2);' };
  const never = /__never_matches__/;
  try {
    const bundle = await rolldown({
      cwd,
      input: 'entry',
      devtools: { mode: 'report' },
      plugins: [
        // First, so that its `resolveId` and `load` are asked before `virtual` answers.
        {
          name: 'filtered',
          resolveId: { filter: { id: never }, handler: () => 'x' },
          load: { filter: { id: never }, handler: () => '' },
          transform: { filter: { id: never }, handler: (code) => `${code};` },
          renderChunk: { filter: { code: never }, handler: (code) => `${code};` },
        },
        {
          name: 'virtual',
          resolveId: (id) => (id in sources ? id : null),
          load: (id) => sources[id],
        },
        { name: 'same-code', transform: (code) => code },
        // A result without `code` keeps the code unchanged.
        { name: 'no-code', transform: () => ({ map: null }) },
        { name: 'changes-dep', transform: (code, id) => (id === 'dep' ? `${code}\n// dep` : null) },
      ],
    });
    await bundle.write({ dir: 'dist' });
    await bundle.close();
    const metrics = JSON.parse(
      readFileSync(join(cwd, 'node_modules/.rolldown/report/dist/metrics.json'), 'utf8'),
    );
    const transform = (name) => metrics.plugins.find((p) => p.name === name).hooks.transform;
    // The runtime module also goes through `transform`, so count at least the two sources.
    const { calls } = transform('same-code');
    expect(calls).toBeGreaterThanOrEqual(2);
    expect(transform('same-code')).toEqual({ calls, ms: expect.any(Number), noopCalls: calls });
    expect(transform('no-code')).toEqual({ calls, ms: expect.any(Number), noopCalls: calls });
    expect(transform('changes-dep')).toEqual({
      calls,
      ms: expect.any(Number),
      noopCalls: calls - 1,
    });
    expect(metrics.plugins.map((p) => p.name)).not.toContain('filtered');
  } finally {
    rmSync(cwd, { recursive: true, force: true });
  }
});

// Each worker builds its own filter from `threadNumber`, and the driver checks the filter before
// a worker takes the call. So the driver may skip a call only when every worker's filter rejects
// it. Worker `k` accepts only `m<k>`, so the report holds one call per worker.
//
// The wasm binding drops parallel plugins, so the WASI suite skips this test.
test.skipIf(isWasiTest)(
  'report mode keeps parallel plugin calls that the filter of any worker accepts',
  async () => {
    // The same worker count as `initializeParallelPlugins`.
    const workerCount = Math.min(availableParallelism(), 8);
    const ids = Array.from({ length: 8 }, (_, i) => `m${i}`);
    const sources = Object.fromEntries(ids.map((id) => [id, `console.log("${id}");`]));
    sources.entry = ids.map((id) => `import "${id}";`).join('\n');
    const threadFilter = defineParallelPlugin(join(import.meta.dirname, 'thread-filter-plugin.js'));
    const cwd = mkdtempSync(join(tmpdir(), 'rolldown-report-parallel-'));
    try {
      const bundle = await rolldown({
        cwd,
        input: 'entry',
        devtools: { mode: 'report' },
        plugins: [
          {
            name: 'virtual',
            resolveId: (id) => (id in sources ? id : null),
            load: (id) => sources[id],
          },
          threadFilter(),
        ],
      });
      await bundle.write({ dir: 'dist' });
      await bundle.close();
      const metrics = JSON.parse(
        readFileSync(join(cwd, 'node_modules/.rolldown/report/dist/metrics.json'), 'utf8'),
      );
      const plugin = metrics.plugins.find((p) => p.name === 'thread-filter');
      expect(plugin.hooks.transform.calls).toBe(workerCount);
    } finally {
      rmSync(cwd, { recursive: true, force: true });
    }
  },
);
