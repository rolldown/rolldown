import assert from 'node:assert';
import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const configName = process.env.SCENARIO_CONFIG ?? globalThis.__configName;
// Code outside the bundle loads the entries: `require()` for CJS output, `import()` for ESM output.
const load = async (name) =>
  configName === 'cjs' ? require(`./dist/${name}.js`) : await import(`./dist/${name}.js`);

const result = (app, seg, d, p) => ({
  dKeys: Object.keys(d).sort(),
  pKeys: Object.keys(p).sort(),
  values: [app.n, seg.first, d.n, p.n, app.reads()],
});

const scenarios = {
  async dynamicFirst() {
    const main = await load('main');
    const d = await main.loadD();
    const app = await load('app');
    const seg = await load('seg');
    const p = await main.loadP();
    return result(app, seg, d, p);
  },
  async appFirst() {
    const app = await load('app');
    const seg = await load('seg');
    const main = await load('main');
    const d = await main.loadD();
    const p = await main.loadP();
    return result(app, seg, d, p);
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
  // The importers of `d.js` cannot read an extracted namespace, because `d.js` re-exports a
  // CommonJS property. Thus the namespace of `d.js` has the namespace of `counter.cjs` as an extra
  // key for `p.js` (issue #11397). The shared read adds no key to this namespace.
  const expected = {
    dKeys: configName === 'cjs' ? ['dval', 'import_counter', 'n'] : ['dval', 'n', 't'],
    pKeys: ['dval', 'n'],
    // The facade, `app.js` and `d.js` share one read. `p.js` keeps its own read, so the getter of
    // `n` runs two times. Node reads `n` one time and gives `1` to each export.
    values: [1, 1, 1, 2, 2],
  };
  assert.deepStrictEqual(run('dynamicFirst'), expected);
  assert.deepStrictEqual(run('appFirst'), expected);
}
