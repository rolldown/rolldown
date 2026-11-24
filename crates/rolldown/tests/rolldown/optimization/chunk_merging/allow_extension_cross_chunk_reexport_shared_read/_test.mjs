import assert from 'node:assert';
import { execFileSync } from 'node:child_process';
import { readdirSync } from 'node:fs';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
const configName = process.env.SCENARIO_CONFIG ?? globalThis.__configName;
// Code outside the bundle loads the entries: `require()` for CJS output, `import()` for ESM output.
const load = async (name) =>
  configName === 'cjs' ? require(`./dist/${name}.js`) : await import(`./dist/${name}.js`);

const scenarios = {
  async appFirst() {
    const app = await load('app');
    const seg = await load('seg');
    return [app.n, seg.first, app.reads(), app.y, app.yReads()];
  },
  async segFirst() {
    const seg = await load('seg');
    const app = await load('app');
    return [seg.first, app.n, app.reads(), app.y, app.yReads()];
  },
  async throwing() {
    const app = await load('app');
    const seg = await load('seg');
    return [typeof app.x, typeof seg.x, app.calls()];
  },
  async writeAfterLoad() {
    const app = await load('app');
    app.setM(2);
    const seg = await load('seg');
    return [app.m, seg.m];
  },
  async writeAfterRead() {
    const app = await load('app');
    const before = app.m;
    app.setM(2);
    const seg = await load('seg');
    return [before, app.m, seg.m];
  },
  async other() {
    const other = await load('other');
    return [other.own, other.reads()];
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
  // `seg.js` keeps its facade. `seg_barrel.js` gets no facade (see `mod.rs`).
  const files = readdirSync(new URL('./dist', import.meta.url));
  assert.deepStrictEqual(
    files.filter((file) => file.startsWith('seg')),
    ['seg.js'],
  );
  // The getters of `n` and `y` count their reads. Each getter runs one time, so both entries get
  // `1` in each load order.
  assert.deepStrictEqual(run('appFirst'), [1, 1, 1, 1, 1]);
  assert.deepStrictEqual(run('segFirst'), [1, 1, 1, 1, 1]);
  // The getter of `x` throws. Both entries get `undefined`, and the getter runs one time.
  assert.deepStrictEqual(run('throwing'), ['undefined', 'undefined', 1]);
  // `mut.cjs` writes `m` after `app` loads. Node reads `m` when it imports `mut.cjs`, and ESM
  // output reads `m` when `app` loads, so both give `[1, 1]`. CJS output reads the properties of
  // `mut.cjs` at the first access of an export of `mut.cjs`. Here that access is the read of
  // `setM` before the write, so CJS output also gives `[1, 1]`. After the first read, both
  // entries keep its value, also after a later write.
  assert.deepStrictEqual(run('writeAfterLoad'), [1, 1]);
  assert.deepStrictEqual(run('writeAfterRead'), [1, 1, 1]);
  // `other` re-exports `reads` of `counter.cjs`. Node reads all properties of `counter.cjs` when
  // it imports the module, so the getter of `n` runs one time. The accessor of the module also
  // reads `n` when `other` reads `reads`.
  assert.deepStrictEqual(run('other'), [17, 1]);
}
