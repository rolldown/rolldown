import assert from 'node:assert';
import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const configName = process.env.SCENARIO_CONFIG ?? globalThis.__configName;
// Code outside the bundle loads the entries: `require()` for CJS output, `import()` for ESM output.
const load = async (name) =>
  configName === 'cjs' ? require(`./dist/${name}.js`) : await import(`./dist/${name}.js`);

// The facade, the `import()` of the entry in the bundle, the export that reads `n`, and the count
// of reads of `n`, by layout.
const layouts = {
  forward: { facade: 'seg', load: 'load', name: 'first', reads: 'reads' },
  owner: { facade: 'seg2', load: 'load2', name: 'second', reads: 'reads2' },
};

// Each value list is `[facade, first read of the namespace, second read of the namespace, reads]`.
const withReads = async (result) => {
  const app = await load('app');
  for (const [layout, names] of Object.entries(layouts)) {
    result[layout].values.push(app[names.reads]());
  }
  return result;
};

const scenarios = {
  async facadeFirst() {
    const main = await load('main');
    const result = {};
    for (const [layout, { facade, load: loadName, name }] of Object.entries(layouts)) {
      const value = (await load(facade))[name];
      const ns = await main[loadName]();
      result[layout] = { keys: Object.keys(ns).sort(), values: [value, ns[name], ns[name]] };
    }
    return withReads(result);
  },
  async namespaceFirst() {
    const main = await load('main');
    const result = {};
    for (const [layout, { facade, load: loadName, name }] of Object.entries(layouts)) {
      const ns = await main[loadName]();
      const first = ns[name];
      const second = ns[name];
      const value = (await load(facade))[name];
      result[layout] = { keys: Object.keys(ns).sort(), values: [value, first, second] };
    }
    return withReads(result);
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
  // The facade and the getter in the namespace object share one read of `n`, so the getter of `n`
  // runs one time and each read gives `1`, as in Node. The namespace object has only the exports
  // of the entry.
  const expected = {
    forward: { keys: ['first', 'foreign'], values: [1, 1, 1, 1] },
    owner: { keys: ['foreign', 'second'], values: [1, 1, 1, 1] },
  };
  assert.deepStrictEqual(run('facadeFirst'), expected);
  assert.deepStrictEqual(run('namespaceFirst'), expected);
}
