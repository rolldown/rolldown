import { execa } from 'execa';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { isWasiTest } from 'rolldown-tests/utils';
import { test } from 'vitest';

// A glob like `@data/*.js` is resolved with `this.resolve`. If the transform blocked a
// worker on that resolve, one worker and a few such modules would leave no core to run
// the nested `this.resolve()` from the JS hook, and the build would hang.
test.skipIf(isWasiTest)(
  'import-glob resolves aliased globs without blocking runtime workers',
  { timeout: 60_000 },
  async () => {
    const root = await mkdtemp(path.join(tmpdir(), 'rolldown-import-glob-resolve-'));
    try {
      await mkdir(path.join(root, 'data'));
      await writeFile(path.join(root, 'data/a.js'), `export default 'globbed-data';\n`);
      const count = 10;
      let main = '';
      for (let i = 0; i < count; i++) {
        await writeFile(
          path.join(root, `m${i}.js`),
          `export default import.meta.glob('@data/*.js', { eager: true });\n`,
        );
        main += `export { default as m${i} } from './m${i}.js';\n`;
      }
      await writeFile(path.join(root, 'main.js'), main);

      await execa(
        process.execPath,
        [fileURLToPath(new URL('./import-glob-resolve.mjs', import.meta.url)), root],
        { timeout: 30_000, env: { ROLLDOWN_WORKER_THREADS: '1' } },
      );
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  },
);
