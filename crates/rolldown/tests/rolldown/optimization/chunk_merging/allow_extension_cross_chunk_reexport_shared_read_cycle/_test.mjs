import assert from 'node:assert';
import { execFileSync } from 'node:child_process';
import { readdirSync } from 'node:fs';

// The test uses only ESM output (see `mod.rs`).
const load = (name) => import(`./dist/${name}.js`);

const scenarios = {
  // `app.js` loads through the `import()` in `main.js`.
  async app() {
    const app = await (await load('main')).loadApp();
    return [globalThis.earlyZ, app.x, app.z];
  },
};

if (process.env.SCENARIO) {
  console.log(JSON.stringify(await scenarios[process.env.SCENARIO]()));
} else {
  const run = (scenario) =>
    JSON.parse(
      execFileSync(process.execPath, [import.meta.filename], {
        env: { ...process.env, SCENARIO: scenario },
        encoding: 'utf8',
      }),
    );
  // The read of `z` in the body of `app.js` gets the value of Node.
  assert.deepStrictEqual(run('app'), ['z', 'x', 'z']);
  // `seg.js` gets no facade (see `mod.rs`).
  const files = readdirSync(new URL('./dist', import.meta.url));
  assert.deepStrictEqual(
    files.filter((file) => file.startsWith('seg')),
    [],
  );
}
