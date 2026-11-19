import assert from 'node:assert';
import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const configName = process.env.SCENARIO_CONFIG ?? globalThis.__configName;
// Code outside the bundle loads the entries: `require()` for CJS output, `import()` for ESM output.
const load = async (name) =>
  configName === 'cjs' ? require(`./dist/${name}.js`) : await import(`./dist/${name}.js`);

// `loadApp` is the `import()` of `app.js` in the bundle.
const scenarios = {
  async appFirst() {
    const app = await (await load('main')).loadApp();
    const n = app.n;
    const seg = await load('seg');
    return { keys: Object.keys(app).sort(), values: [seg.first, n, app.reads()] };
  },
  async segFirst() {
    const seg = await load('seg');
    const app = await (await load('main')).loadApp();
    return { keys: Object.keys(app).sort(), values: [seg.first, app.n, app.reads()] };
  },
  // Only the `app.js` entry loads, and code reads `app.n` two times.
  async appOnly() {
    const app = await (await load('main')).loadApp();
    return { keys: Object.keys(app).sort(), values: [app.n, app.n, app.reads()] };
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
  // The facade reads `n` through the public export `n` of the `app.js` chunk, so the namespace of
  // `app.js` gets no new key. The getter of `n` runs one time, and each read gives `1`, as in Node.
  // In CJS output, the getter of `app.n` saves the value of its first read.
  const expected = { keys: ['n', 'reads'], values: [1, 1, 1] };
  assert.deepStrictEqual(run('appFirst'), expected);
  assert.deepStrictEqual(run('segFirst'), expected);
  assert.deepStrictEqual(run('appOnly'), expected);
}
