import { rolldown } from 'rolldown';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { expect, test } from 'vitest';

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
