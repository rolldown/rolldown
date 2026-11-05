import { execFileSync } from 'node:child_process';
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

import type { OutputOptions } from 'rolldown';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

// The lazy consumer rule can move the runtime into the entry `h`, which loads the lazy chunk `d`.
// Then `d` imports `h`. A new importer of `h` waits for each cycle that contains `h`, so the rule
// accepts `h` only when each module that `h` reaches is synchronous. A `node:` module is
// synchronous, but `output.paths` can send its import to another module after the fold. Here that
// module imports `h` and waits until `d` runs. So a fold into `h` deadlocks, and Node exits with
// code 13. See internal-docs/code-splitting/design.md#runtime-placement.
const forms: { name: string; paths: (ext: string) => OutputOptions['paths'] }[] = [
  { name: 'map', paths: (ext) => ({ 'node:fs': ext }) },
  { name: 'function', paths: (ext) => (id) => (id === 'node:fs' ? ext : id) },
];

test.each(forms)('output.paths can send a node: import to an async module: $name', async (form) => {
  const directory = await mkdtemp(path.join(tmpdir(), 'rolldown-node-external-paths-'));
  try {
    const source = path.join(directory, 'source');
    const output = path.join(directory, 'output');
    await mkdir(source);
    const files: Record<string, string> = {
      // `h` and `d` use a helper, so both import the runtime. `x` uses no helper.
      'h.js': `import 'node:fs';
import * as hn from './hn.js';
console.log('h', Object.keys(hn).join());
import('./d.js').then(() => console.log('d loaded'));
`,
      'hn.js': 'export const q = 1;\n',
      'd.js': `import * as dn from './dn.js';
console.log('d', Object.keys(dn).join());
globalThis.resolveE?.();
`,
      'dn.js': 'export const z = 1;\n',
      'x.js': "import 'node:fs';\nconsole.log('x');\n",
      // The external starts no timer, so a deadlock makes Node exit.
      'ext.mjs': `import '../output/h.mjs';
console.log('e start');
await new Promise((resolve) => {
  globalThis.resolveE = resolve;
});
console.log('e done');
`,
    };
    await Promise.all(
      Object.entries(files).map(([name, code]) => writeFile(path.join(source, name), code)),
    );
    const build = await rolldown({
      cwd: source,
      input: { h: 'h.js', x: 'x.js' },
      external: ['node:fs'],
    });
    try {
      await build.write({
        dir: output,
        format: 'esm',
        entryFileNames: '[name].mjs',
        chunkFileNames: '[name]-[hash].mjs',
        paths: form.paths(pathToFileURL(path.join(source, 'ext.mjs')).href),
      });
    } finally {
      await build.close();
    }
    // Only `x.mjs` runs. A run of `h.mjs` deadlocks on each layout, because `h` calls `import()`
    // only after the external completes.
    const run = execFileSync(process.execPath, [path.join(output, 'x.mjs')], {
      encoding: 'utf8',
      timeout: 10_000,
    });
    // The code forces the order of these four lines. The order of `x` and `d loaded` depends on the
    // microtask order.
    expect(run.trim().split('\n').slice(0, 4)).toEqual(['h q', 'e start', 'd z', 'e done']);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
