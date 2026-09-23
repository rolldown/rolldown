// The `ci: webcontainer` suite installs @rolldown/binding-wasm32-wasi next to `rolldown`, so the
// generated loader never reaches the WebContainer fallback. vite.new has no such package and loads
// the binding `webcontainer-fallback.cjs` installs instead, which is the path #10938 broke.
//
// Plain Node stands in for WebContainer: the loader keys on `process.versions.webcontainer`, and
// the fallback skips its download once its install directory holds the binding, so the packed
// binding is seeded there and the packed `rolldown` is installed with no binding at all.
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { afterAll, expect, inject, test } from 'vitest';

const fixtures = path.resolve('tests/fixtures/node');
const version = inject('rolldownVersion');

// the literal directory webcontainer-fallback.cjs installs into, not os.tmpdir()
const fallbackDir = `/tmp/rolldown-${version}`;
const bindingEntry = path.join(
  fallbackDir,
  'node_modules/@rolldown/binding-wasm32-wasi/rolldown-binding.wasi.cjs',
);

const app = fs.mkdtempSync(path.join(os.tmpdir(), 'rolldown-webcontainer-fallback-'));

const PROBE = `
import { createRequire } from 'node:module';

process.versions.webcontainer = '1';

const { rolldown } = await import('rolldown');
const bundle = await rolldown({ input: 'index.js', logLevel: 'silent' });
await bundle.generate({ format: 'esm' });
await bundle.close();

const require = createRequire(import.meta.url);
console.log(JSON.stringify(Object.keys(require.cache).filter((id) => id.endsWith('rolldown-binding.wasi.cjs'))));
`;

function run(command: string, args: string[], cwd: string) {
  const result = spawnSync(command, args, { cwd, encoding: 'utf8' });
  if (result.status !== 0) {
    throw new Error(
      `\`${command} ${args.join(' ')}\` in ${cwd} exited with ${result.status ?? result.error?.message}\n${result.stdout}${result.stderr}`,
    );
  }
  return result.stdout;
}

afterAll(() => {
  fs.rmSync(app, { recursive: true, force: true });
  fs.rmSync(fallbackDir, { recursive: true, force: true });
});

test('the packed rolldown loads the packed WASI binding through its WebContainer fallback', () => {
  fs.rmSync(fallbackDir, { recursive: true, force: true });
  fs.mkdirSync(fallbackDir, { recursive: true });
  run('pnpm', ['add', path.join(fixtures, 'rolldown-binding-wasm32-wasi.tgz')], fallbackDir);
  expect(fs.existsSync(bindingEntry)).toBe(true);

  // no optional dependencies, so no native binding can be found and only the fallback is left
  fs.writeFileSync(path.join(app, 'package.json'), '{}');
  fs.writeFileSync(path.join(app, '.npmrc'), 'optional=false\n');
  run('pnpm', ['add', path.join(fixtures, 'rolldown.tgz')], app);

  fs.writeFileSync(path.join(app, 'index.js'), 'export const value = 1;\n');
  fs.writeFileSync(path.join(app, 'probe.mjs'), PROBE);
  const loaded = JSON.parse(run(process.execPath, ['probe.mjs'], app));

  // require.cache keys are real paths, and pnpm links the package out of its store
  expect(loaded).toEqual([fs.realpathSync(bindingEntry)]);
});
