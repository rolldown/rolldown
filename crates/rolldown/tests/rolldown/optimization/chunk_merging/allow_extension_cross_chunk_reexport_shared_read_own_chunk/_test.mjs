import assert from 'node:assert';
import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const configName = process.env.SCENARIO_CONFIG ?? globalThis.__configName;
// Code outside the bundle loads the entries: `require()` for CJS output, `import()` for ESM output.
const load = async (name) =>
  configName === 'cjs' ? require(`./dist/${name}.js`) : await import(`./dist/${name}.js`);

const scenarios = {
  async appFirst() {
    const app = await load('app');
    const seg = await load('seg');
    return [app.n, seg.first, app.reads()];
  },
  async segFirst() {
    const seg = await load('seg');
    const app = await load('app');
    return [seg.first, app.n, app.reads()];
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
  // The getter of `n` counts its reads. It runs one time, so both entries get `1` in each load
  // order.
  assert.deepStrictEqual(run('appFirst'), [1, 1, 1]);
  assert.deepStrictEqual(run('segFirst'), [1, 1, 1]);
}
