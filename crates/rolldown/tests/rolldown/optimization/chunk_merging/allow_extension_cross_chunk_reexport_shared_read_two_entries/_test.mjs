import assert from 'node:assert';
import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const configName = process.env.SCENARIO_CONFIG ?? globalThis.__configName;
// Code outside the bundle loads the entries: `require()` for CJS output, `import()` for ESM output.
const load = async (name) =>
  configName === 'cjs' ? require(`./dist/${name}.js`) : await import(`./dist/${name}.js`);

// `loadA` and `loadB` are the `import()` of `a.js` and `b.js` in the bundle. `b.js` counts its
// runs in `globalThis.bRuns`.
const scenarios = {
  async facadeOnly() {
    const seg = await load('seg');
    return { values: [seg.first, seg.aval], bRuns: globalThis.bRuns };
  },
  async aFirst() {
    const main = await load('main');
    const a = await main.loadA();
    const n = a.n;
    const seg = await load('seg');
    const b = await main.loadB();
    return {
      keys: [Object.keys(a).sort(), Object.keys(b).sort()],
      values: [n, seg.first, b.n, b.reads()],
    };
  },
  async bFirst() {
    const main = await load('main');
    const b = await main.loadB();
    const n = b.n;
    const seg = await load('seg');
    const a = await main.loadA();
    return {
      keys: [Object.keys(a).sort(), Object.keys(b).sort()],
      values: [n, seg.first, a.n, b.reads()],
    };
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
  // A load of the facade alone runs `b.js` one time, as in Node.
  assert.deepStrictEqual(run('facadeOnly'), { values: [1, 1], bRuns: 1 });
  // The getter of `n` runs one time, and each read gives `1`, as in Node. The namespaces of `a.js`
  // and `b.js` get no new key.
  const expected = {
    keys: [
      ['aval', 'n'],
      ['n', 'reads'],
    ],
    values: [1, 1, 1, 1],
  };
  assert.deepStrictEqual(run('aFirst'), expected);
  assert.deepStrictEqual(run('bFirst'), expected);
}
