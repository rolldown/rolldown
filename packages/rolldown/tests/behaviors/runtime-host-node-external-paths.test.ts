import { execFileSync } from 'node:child_process';
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

import type { OutputOptions } from 'rolldown';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

// Condition 4 of the host rule: no external module whose code rolldown cannot know runs before the
// host completes. A `node:` module imports no user module, so it passes, but `output.paths` can
// send its import to another module after the fold. Here `h` and `d` use helpers, and `d` imports
// `h`, so `h` can host the runtime. `output.paths` sends the `node:fs` import of `h` to `ext.mjs`,
// which imports the entry `d`. So when `h` loads first, the code of `d` runs before the code of
// `h`, and it calls `__exportAll`. As the runtime host, `h` declares `__exportAll` in its code, so
// the call fails. See internal-docs/code-splitting/design.md#external-modules.
const forms: { name: string; paths: (ext: string) => OutputOptions['paths'] }[] = [
  { name: 'map', paths: (ext) => ({ 'node:fs': ext }) },
  { name: 'function', paths: (ext) => (id) => (id === 'node:fs' ? ext : id) },
];

test.each(forms)(
  'output.paths can send a node: import to a module that imports an entry: $name',
  async (form) => {
    const directory = await mkdtemp(path.join(tmpdir(), 'rolldown-node-external-paths-'));
    try {
      const source = path.join(directory, 'source');
      const output = path.join(directory, 'output');
      await mkdir(source);
      const files: Record<string, string> = {
        'h.js': `import 'node:fs';
import * as hn from './hn.js';
console.log('h', Object.keys(hn).join());
`,
        'hn.js': 'export const q = 1;\n',
        'd.js': `import './h.js';
import * as dn from './dn.js';
console.log('d', Object.keys(dn).join());
`,
        'dn.js': 'export const z = 1;\n',
        // When `h` loads first, this import closes the cycle `h` -> `ext.mjs` -> `d` -> `h`.
        'ext.mjs': `import '../output/d.mjs';
console.log('e');
`,
      };
      await Promise.all(
        Object.entries(files).map(([name, code]) => writeFile(path.join(source, name), code)),
      );
      const build = await rolldown({
        cwd: source,
        input: { h: 'h.js', d: 'd.js' },
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
      const run = execFileSync(process.execPath, [path.join(output, 'h.mjs')], {
        encoding: 'utf8',
        timeout: 10_000,
      });
      expect(run.trim().split('\n')).toEqual(['d z', 'e', 'h q']);
    } finally {
      await rm(directory, { recursive: true, force: true });
    }
  },
);
