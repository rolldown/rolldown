import assert from 'node:assert';
import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const configName = process.env.SCENARIO_CONFIG ?? globalThis.__configName;
// Code outside the bundle loads the entries: `require()` for CJS output, `import()` for ESM output.
const load = async (name) =>
  configName === 'cjs' ? require(`./dist/${name}.js`) : await import(`./dist/${name}.js`);

const scenarios = {
  // Only the `app.js` entry loads, through the `import()` in `main.js`.
  async appOnly() {
    const app = await (await load('main')).loadApp();
    return [Object.keys(app).sort(), app.x, app.z];
  },
  async segFirst() {
    const seg = await load('seg');
    const seg2 = await load('seg2');
    const app = await load('app');
    return [seg.y, seg2.z, app.x, app.z];
  },
  async seg2First() {
    const seg2 = await load('seg2');
    const seg = await load('seg');
    const app = await load('app');
    return [seg.y, seg2.z, app.x, app.z];
  },
  async facades() {
    const seg = await load('seg');
    const seg2 = await load('seg2');
    return [Object.keys(seg).sort(), seg.foreign, Object.keys(seg2).sort(), seg2.foreign2];
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
  // Node reads `x`, `z` and `y` in this order when the first entry loads, so each load order gives
  // the same values. The namespace of `app.js` gets no new key.
  assert.deepStrictEqual(run('appOnly'), [['x', 'z'], 1, 2]);
  assert.deepStrictEqual(run('segFirst'), [3, 2, 1, 2]);
  assert.deepStrictEqual(run('seg2First'), [3, 2, 1, 2]);
  // Both facades keep their exports.
  assert.deepStrictEqual(run('facades'), [['foreign', 'y'], 5, ['foreign2', 'z'], 5]);
}
