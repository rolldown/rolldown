import assert from 'node:assert';
import { execFileSync } from 'node:child_process';
import { readdirSync } from 'node:fs';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const configName = process.env.SCENARIO_CONFIG ?? globalThis.__configName;
// Code outside the bundle loads the entries: `require()` for CJS output, `import()` for ESM output.
const load = async (name) =>
  configName === 'cjs' ? require(`./dist/${name}.js`) : await import(`./dist/${name}.js`);
// The CommonJS modules with a known order of `x` and `y`.
const modules = ['xy', 'yx', 'defxy', 'defyx'];
const values = (entry, suffix) => Object.fromEntries(modules.map((m) => [m, entry[m + suffix]]));
// The number of getter calls of each module. A module that did not run has no counter.
const counts = () => Object.fromEntries(modules.map((m) => [m, globalThis[`${m}Count`]?.() ?? 0]));

// Each scenario reads the exports of an entry immediately after it loads the entry. A CJS export
// reads the property when code reads the export, so the reads occur in the order of the loads.
const scenarios = {
  async appFirst() {
    const app = await load('app');
    const x = values(app, 'X');
    const seg = await load('seg');
    return { x, y: values(seg, 'Y'), count: counts() };
  },
  async segFirst() {
    const seg = await load('seg');
    const y = values(seg, 'Y');
    const app = await load('app');
    return { x: values(app, 'X'), y, count: counts() };
  },
  async appOnly() {
    const app = await load('app');
    return { x: values(app, 'X'), count: counts() };
  },
  async segOnly() {
    const seg = await load('seg');
    return { y: values(seg, 'Y'), count: counts() };
  },
  // `side` imports only an ES module. Its chunk loads the chunks of the accessors, but it reads no
  // CommonJS property.
  async sideFirst() {
    const side = await load('side');
    const sideCount = counts();
    const seg = await load('seg');
    const y = values(seg, 'Y');
    const segCount = counts();
    const app = await load('app');
    const x = values(app, 'X');
    const again = { x: values(app, 'X'), y: values(seg, 'Y') };
    return { own: side.own, sideCount, y, segCount, x, again, count: counts() };
  },
  async segKeys() {
    const seg = await load('seg');
    return [Object.keys(seg).sort(), seg.foreign];
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
  // Node reads `x` and then `y` of `xy.cjs` and `defxy.cjs`, and `y` and then `x` of `yx.cjs` and
  // `defyx.cjs`. The values do not depend on the entry that loads first.
  const x = { xy: 1, yx: 2, defxy: 1, defyx: 2 };
  const y = { xy: 2, yx: 1, defxy: 2, defyx: 1 };
  const count = { xy: 2, yx: 2, defxy: 2, defyx: 2 };
  assert.deepStrictEqual(run('appFirst'), { x, y, count });
  assert.deepStrictEqual(run('segFirst'), { x, y, count });
  // An entry that reads one property of a module also reads the other property, as in Node.
  assert.deepStrictEqual(run('appOnly'), { x, count });
  assert.deepStrictEqual(run('segOnly'), { y, count });
  // The load of an accessor does not read a property. The first call reads all properties, and
  // later reads use the values of that call.
  const none = { xy: 0, yx: 0, defxy: 0, defyx: 0 };
  assert.deepStrictEqual(run('sideFirst'), {
    own: 17,
    sideCount: none,
    y,
    segCount: count,
    x,
    again: { x, y },
    count,
  });
  assert.deepStrictEqual(run('segKeys'), [['defxyY', 'defyxY', 'foreign', 'xyY', 'yxY'], 5]);
  // `seg_unknown.js` gets no facade (see `mod.rs`).
  const files = readdirSync(new URL('./dist', import.meta.url));
  assert.deepStrictEqual(
    files.filter((file) => file.startsWith('seg')),
    ['seg.js'],
  );
}
