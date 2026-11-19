import assert from 'node:assert';
import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const configName = process.env.SCENARIO_CONFIG ?? globalThis.__configName;
// Code outside the bundle loads the entries: `require()` for CJS output, `import()` for ESM output.
const load = async (name) =>
  configName === 'cjs' ? require(`./dist/${name}.js`) : await import(`./dist/${name}.js`);

// `loadApp` is the `import()` of `app.js` in the bundle. Each scenario reads `app.n` one time.
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
  // The namespace of `app.js` has the namespace of `counter.cjs` as an extra key, because the
  // facade reads `n` from it. The `app.js` chunk gets no forward, so the namespace gets no other
  // key.
  const keys = configName === 'cjs' ? ['import_counter', 'n', 'reads'] : ['n', 'reads', 't'];
  // The facade and the `app.js` chunk each read `n`, so the getter of `n` runs two times. Node
  // reads `n` one time and gives `1` to each export. The facade reads `n` when it loads. In ESM
  // output, the `app.js` chunk reads `n` when it loads, before the facade that imports it. In CJS
  // output, the `app.js` chunk reads `n` when code reads `app.n`.
  assert.deepStrictEqual(run('appFirst'), { keys, values: [2, 1, 2] });
  assert.deepStrictEqual(run('segFirst'), {
    keys,
    values: configName === 'cjs' ? [1, 2, 2] : [2, 1, 2],
  });
}
