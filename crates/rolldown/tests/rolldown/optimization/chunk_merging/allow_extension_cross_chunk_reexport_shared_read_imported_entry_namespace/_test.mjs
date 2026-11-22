import assert from 'node:assert';
import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const configName = process.env.SCENARIO_CONFIG ?? globalThis.__configName;
// Code outside the bundle loads the entries: `require()` for CJS output, `import()` for ESM output.
const load = async (name) =>
  configName === 'cjs' ? require(`./dist/${name}.js`) : await import(`./dist/${name}.js`);

// `loadApp` and `loadSeg` are the `import()` of `app.js` and `seg.js` in the bundle. Each step
// reads `n` one time: `app.n`, `first` of the namespace object of `seg.js`, or `first` of the
// facade.
const steps = {
  app: async (main) => (await main.loadApp()).n,
  ns: async (main) => (await main.loadSeg()).first,
  facade: async () => (await load('seg')).first,
};

const runSteps = async (names) => {
  const main = await load('main');
  const values = [];
  for (const name of names) values.push(await steps[name](main));
  const app = await main.loadApp();
  return {
    keys: Object.keys(app).sort(),
    nsKeys: Object.keys(await main.loadSeg()).sort(),
    values: [...values, app.reads()],
  };
};

if (process.env.SCENARIO) {
  console.log(JSON.stringify(await runSteps(process.env.SCENARIO.split('-'))));
} else {
  // Each scenario runs in a new process, so it gets new CommonJS modules.
  const run = (scenario) =>
    JSON.parse(
      execFileSync(process.execPath, [import.meta.filename], {
        env: { ...process.env, SCENARIO: scenario, SCENARIO_CONFIG: configName ?? '' },
        encoding: 'utf8',
      }),
    );
  // The `app.js` chunk exports the namespace of `counter.cjs` under an internal name, so the
  // namespace of `app.js` has one extra key: `t` in ESM output, `import_counter` in CJS output.
  const keys = configName === 'cjs' ? ['import_counter', 'n', 'reads'] : ['n', 'reads', 't'];
  // The getter of `n` runs one time, and each read gives `1`, as in Node.
  for (const scenario of ['ns-app', 'app-ns', 'facade-ns-app', 'ns-ns-app']) {
    const values = scenario.split('-').map(() => 1);
    assert.deepStrictEqual(run(scenario), {
      keys,
      nsKeys: ['first', 'foreign'],
      values: [...values, 1],
    });
  }
}
