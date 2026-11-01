import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { rolldown } from 'rolldown';
import { viteDynamicImportVarsPlugin, viteImportGlobPlugin } from 'rolldown/experimental';
import { expect, test } from 'vitest';

test.for([
  { directory: 'modules', query: '?value="quoted"' },
  { directory: 'modules', query: '?value="line\nbreak"' },
  { directory: 'a"b', query: '', skip: process.platform === 'win32' },
])(
  'escapes dynamic import glob strings ($directory, $query)',
  async ({ directory, query, skip }, context) => {
    if (skip) context.skip();
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'rolldown-dynamic-import-escaping-'));
    fs.mkdirSync(path.join(dir, directory));
    fs.writeFileSync(path.join(dir, directory, 'test.js'), 'export default 42;');
    const input = path.join(dir, 'main.js');
    fs.writeFileSync(
      input,
      'export const load = async (name) => (await import(`./' +
        directory +
        '/${name}.js' +
        query +
        '`)).default;',
    );
    const seenQueries: string[] = [];
    const build = await rolldown({
      cwd: dir,
      input,
      plugins: [
        viteDynamicImportVarsPlugin(),
        viteImportGlobPlugin(),
        {
          name: 'query-module',
          resolveId(id) {
            const queryIndex = id.indexOf('?');
            if (queryIndex === -1) return;
            seenQueries.push(id.slice(queryIndex));
            return '\0' + id;
          },
          load(id) {
            if (id.startsWith('\0') && id.includes('?')) return 'export default 42;';
          },
        },
      ],
    });
    try {
      const { output } = await build.generate({ format: 'esm', codeSplitting: false });
      const module = await import(`data:text/javascript,${encodeURIComponent(output[0].code)}`);
      expect(await module.load('test')).toBe(42);
      expect(seenQueries).toEqual(query ? [query] : []);
    } finally {
      await build.close();
      fs.rmSync(dir, { recursive: true, force: true });
    }
  },
);
