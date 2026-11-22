import assert from 'node:assert';
import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const configName = process.env.SCENARIO_CONFIG ?? globalThis.__configName;
// Code outside the bundle loads the entries: `require()` for CJS output, `import()` for ESM output.
const load = async (name) =>
  configName === 'cjs' ? require(`./dist/${name}.js`) : await import(`./dist/${name}.js`);
// The `import()` of `seg2.js` in the bundle. It gives the namespace object of `seg2.js` from the
// `group2` chunk.
const loadNamespace = async () => (await load('main')).loadSeg2();

const scenarios = {
  // Only the `app.js` entry loads.
  async appOnly() {
    const app = await load('app');
    return [app.x, app.z];
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
  // The namespace object of `seg2.js` loads before the `app.js` entry.
  async namespaceFirst() {
    const z = (await loadNamespace()).z;
    const app = await load('app');
    return [z, app.x, app.z];
  },
  // The namespace object of `seg2.js` loads after the `app.js` entry.
  async namespaceLast() {
    const app = await load('app');
    const values = [app.x, app.z];
    return [(await loadNamespace()).z, ...values];
  },
  async keys() {
    const seg = await load('seg');
    const seg2 = await load('seg2');
    const namespace = await loadNamespace();
    return [
      Object.keys(seg).sort(),
      seg.foreign,
      Object.keys(seg2).sort(),
      seg2.foreign2,
      Object.keys(namespace).sort(),
    ];
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
  // the same values.
  assert.deepStrictEqual(run('appOnly'), [1, 2]);
  assert.deepStrictEqual(run('segFirst'), [3, 2, 1, 2]);
  assert.deepStrictEqual(run('seg2First'), [3, 2, 1, 2]);
  assert.deepStrictEqual(run('namespaceFirst'), [2, 1, 2]);
  assert.deepStrictEqual(run('namespaceLast'), [2, 1, 2]);
  // The facades and the namespace object keep their keys.
  assert.deepStrictEqual(run('keys'), [
    ['foreign', 'y'],
    5,
    ['foreign2', 'z'],
    5,
    ['foreign2', 'z'],
  ]);
}
