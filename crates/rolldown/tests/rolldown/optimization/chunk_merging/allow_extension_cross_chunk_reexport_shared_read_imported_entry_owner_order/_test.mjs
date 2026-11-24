import assert from 'node:assert';
import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const configName = process.env.SCENARIO_CONFIG ?? globalThis.__configName;
// Code outside the bundle loads the entries: `require()` for CJS output, `import()` for ESM output.
const load = async (name) =>
  configName === 'cjs' ? require(`./dist/${name}.js`) : await import(`./dist/${name}.js`);
// `loadApp` is the `import()` of `app.js` in the bundle.
const loadApp = async () => (await load('main')).loadApp();

const scenarios = {
  // Only the `app.js` entry loads, and code reads `x` before `z`.
  async appOnly() {
    const app = await loadApp();
    return [Object.keys(app).sort(), app.x, app.z];
  },
  async appFirst() {
    const app = await loadApp();
    const values = [app.x, app.z];
    const seg = await load('seg');
    return [...values, seg.z];
  },
  async segFirst() {
    const seg = await load('seg');
    const z = seg.z;
    const app = await loadApp();
    return [z, app.x, app.z, Object.keys(seg).sort()];
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
  // Node reads `z`, then `x`, when the first entry loads, so each load order gives the same
  // values. The namespace of `app.js` gets no new key.
  assert.deepStrictEqual(run('appOnly'), [['x', 'z'], 2, 1]);
  assert.deepStrictEqual(run('appFirst'), [2, 1, 1]);
  assert.deepStrictEqual(run('segFirst'), [1, 2, 1, ['foreign', 'z']]);
}
