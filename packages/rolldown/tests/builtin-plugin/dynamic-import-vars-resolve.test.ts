import { execa } from 'execa';
import { mkdir, mkdtemp, realpath, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { isWasiTest } from 'rolldown-tests/utils';
import { test } from 'vitest';

// A dynamic import like `@data/${name}.js` is resolved with the JS `resolver`. If the transform
// blocked a worker on that resolver, one worker and a few such modules would leave no core to
// run it, and the build would hang.
test.skipIf(isWasiTest)(
  'dynamic-import-vars resolves aliased globs without blocking runtime workers',
  { timeout: 60_000 },
  async () => {
    const root = await realpath(
      await mkdtemp(path.join(tmpdir(), 'rolldown-dynamic-import-vars-resolve-')),
    );
    try {
      await mkdir(path.join(root, 'data'));
      await writeFile(path.join(root, 'data/a.js'), `export default 'a';\n`);
      const count = 10;
      let main = '';
      for (let i = 0; i < count; i++) {
        await writeFile(
          path.join(root, `m${i}.js`),
          'export default (name) => import(`@data/${name}.js`);\n',
        );
        main += `export { default as m${i} } from './m${i}.js';\n`;
      }
      await writeFile(path.join(root, 'main.js'), main);

      await execa(
        process.execPath,
        [fileURLToPath(new URL('./dynamic-import-vars-resolve.mjs', import.meta.url)), root],
        { timeout: 30_000, env: { ROLLDOWN_WORKER_THREADS: '1' } },
      );
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  },
);
