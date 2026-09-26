import { execFile } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { promisify } from 'node:util';
import { expect, test } from 'vitest';

const execFileAsync = promisify(execFile);

// This test covers the regression in
// https://github.com/rolldown/rolldown/issues/10664.
//
// The plugin called the JS `resolver` from a blocked tokio worker thread.
// `block_in_place` moved that worker's scheduler work to the blocking pool.
// `ROLLDOWN_MAX_BLOCKING_THREADS` limits that pool. More than
// `worker_threads + max_blocking_threads` modules then blocked at the same time.
// No thread remained to run the scheduler work, and the build never finished.
//
// The runtime reads both limits when the binding module loads. This test
// therefore runs a child process. The test allows one worker thread and one
// blocking thread, so the old code could block only 2 hooks at the same time.
// `MODULE_COUNT` is much higher than 2.
const MODULE_COUNT = 20;
// A run that passes takes less than one second. The large timeout only covers a
// slow CI machine.
const TIMEOUT_MS = 30_000;

test(
  'resolver does not deadlock the runtime when worker threads are scarce',
  async () => {
    const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'rolldown-10664-'));
    try {
      await fs.mkdir(path.join(dir, 'data'), { recursive: true });
      await fs.writeFile(path.join(dir, 'data', 'a.js'), 'export default "a";\n');

      let entry = '';
      for (let i = 0; i < MODULE_COUNT; i++) {
        await fs.writeFile(
          path.join(dir, `mod${i}.js`),
          // Only a bare specifier sends the glob to the JS resolver.
          `export const load${i} = (name) => import(\`$lib/data/\${name}.js\`);\n`,
        );
        entry += `export { load${i} } from './mod${i}.js';\n`;
      }
      await fs.writeFile(path.join(dir, 'entry.js'), entry);

      // The script is in a temporary directory. It cannot resolve `rolldown`
      // by name.
      const rolldownUrl = import.meta.resolve('rolldown');
      const experimentalUrl = import.meta.resolve('rolldown/experimental');

      const script = `
import { rolldown } from ${JSON.stringify(rolldownUrl)}
import { viteDynamicImportVarsPlugin } from ${JSON.stringify(experimentalUrl)}
import path from 'node:path'

const dir = ${JSON.stringify(dir)}
const bundle = await rolldown({
  input: path.join(dir, 'entry.js'),
  plugins: [
    viteDynamicImportVarsPlugin({
      async resolver(id) {
        // This await gives control to the event loop. It makes the deadlock
        // window wider.
        await new Promise((r) => setTimeout(r, 5))
        return path.join(dir, id.slice('$lib/'.length))
      },
    }),
    {
      name: 'alias',
      resolveId(id) {
        return id.startsWith('$lib/') ? path.join(dir, id.slice('$lib/'.length)) : null
      },
    },
  ],
})
await bundle.generate({})
await bundle.close()
console.log('done')
`;
      const scriptPath = path.join(dir, 'build.mjs');
      await fs.writeFile(scriptPath, script);

      const { stdout } = await execFileAsync(process.execPath, [scriptPath], {
        cwd: path.dirname(import.meta.dirname),
        timeout: TIMEOUT_MS,
        env: {
          ...process.env,
          ROLLDOWN_WORKER_THREADS: '1',
          ROLLDOWN_MAX_BLOCKING_THREADS: '1',
        },
      });
      expect(stdout).toContain('done');
    } finally {
      await fs.rm(dir, { recursive: true, force: true });
    }
  },
  TIMEOUT_MS,
);
