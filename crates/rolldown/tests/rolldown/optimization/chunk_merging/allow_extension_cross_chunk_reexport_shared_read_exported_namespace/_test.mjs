import assert from 'node:assert';
import { execFileSync } from 'node:child_process';
import { readdirSync } from 'node:fs';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const configName = process.env.SCENARIO_CONFIG ?? globalThis.__configName;
// Code outside the bundle loads the entries: `require()` for CJS output, `import()` for ESM output.
const load = async (name) =>
  configName === 'cjs' ? require(`./dist/${name}.js`) : await import(`./dist/${name}.js`);

const scenarios = {
  // Code outside the bundle loads `app.js`. It reads `x`, then `y` of `counter_a.cjs`, and then
  // the same properties of `counter_b.cjs`.
  async app() {
    const app = await load('app');
    return [app.lib.x, app.lib.y, ...app.readOther()];
  },
};

if (process.env.SCENARIO) {
  console.log(JSON.stringify(await scenarios[process.env.SCENARIO]()));
} else {
  // Each scenario runs in a new process, so it gets new CommonJS modules.
  const run = (scenario) =>
    JSON.parse(
      execFileSync(process.execPath, [import.meta.filename], {
        env: { ...process.env, SCENARIO: scenario, SCENARIO_CONFIG: configName ?? '' },
        encoding: 'utf8',
      }),
    );
  // Each read gets the value of Node.
  assert.deepStrictEqual(run('app'), [1, 2, 1, 2]);
  // The entries get no facade (see `mod.rs`).
  const files = readdirSync(new URL('./dist', import.meta.url));
  assert.deepStrictEqual(
    files.filter((file) => file.startsWith('seg')),
    [],
  );
}
