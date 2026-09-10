import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { rolldown } from 'rolldown';
import { expect, test } from 'vitest';

interface BuildResult {
  calls: { resolve: number; load: number; transform: number };
  code: string;
  mappings: string;
}

// Fixtures live in an OS tempdir: other behaviors test files build into the
// shared `dist/` directory concurrently and could wipe fixtures placed there.
function setupFixture(name: string): { cwd: string; cacheDir: string } {
  const root = mkdtempSync(join(tmpdir(), `rolldown-build-cache-${name}-`));
  const cwd = join(root, 'src');
  const cacheDir = join(root, 'cache');
  mkdirSync(cwd, { recursive: true });
  writeFileSync(
    join(cwd, 'entry.js'),
    `import { dep } from './dep.js';\nexport const out = '__MARKER__' + dep;\n`,
  );
  writeFileSync(join(cwd, 'dep.js'), `export const dep = '__MARKER__ dep';\n`);
  return { cwd, cacheDir };
}

// Counts JS plugin hook invocations for fixture modules; the virtual runtime
// module runs its hooks on every build and is excluded.
async function build(cwd: string, cacheDir: string, key?: string): Promise<BuildResult> {
  const calls = { resolve: 0, load: 0, transform: 0 };
  const bundler = await rolldown({
    input: join(cwd, 'entry.js'),
    cwd,
    experimental: { buildCache: { dir: cacheDir, key } },
    plugins: [
      {
        name: 'counting',
        resolveId(_source, importer) {
          if (importer != null) calls.resolve++;
          return null;
        },
        load(id) {
          if (id.endsWith('.js') && !id.startsWith('\0')) calls.load++;
          return null;
        },
        transform(code, id) {
          if (!id.endsWith('.js') || id.startsWith('\0')) return null;
          calls.transform++;
          return { code: code.replace('__MARKER__', 'transformed'), map: null };
        },
      },
    ],
  });
  const { output } = await bundler.generate({ sourcemap: true });
  await bundler.close();
  const chunk = output[0];
  return { calls, code: chunk.code, mappings: chunk.map!.mappings };
}

test('warm build skips JS resolve/load/transform hooks and emits identical output', async () => {
  const { cwd, cacheDir } = setupFixture('hit');

  const cold = await build(cwd, cacheDir);
  expect(cold.calls).toEqual({ resolve: 1, load: 2, transform: 2 });
  expect(cold.code).toContain('transformed');

  const warm = await build(cwd, cacheDir);
  expect(warm.calls).toEqual({ resolve: 0, load: 0, transform: 0 });
  expect(warm.code).toBe(cold.code);
  expect(warm.mappings).toBe(cold.mappings);
});

test('changing the cache key re-runs the whole pipeline', async () => {
  const { cwd, cacheDir } = setupFixture('key');

  const cold = await build(cwd, cacheDir, 'config-a');
  expect(cold.calls.transform).toBe(2);

  const sameKey = await build(cwd, cacheDir, 'config-a');
  expect(sameKey.calls.transform).toBe(0);

  const newKey = await build(cwd, cacheDir, 'config-b');
  expect(newKey.calls.transform).toBe(2);
});

test('editing a module re-runs only its own pipeline', async () => {
  const { cwd, cacheDir } = setupFixture('edit');

  const cold = await build(cwd, cacheDir);
  expect(cold.calls.transform).toBe(2);

  writeFileSync(join(cwd, 'dep.js'), `export const dep = '__MARKER__ dep edited';\n`);
  const afterEdit = await build(cwd, cacheDir);
  expect(afterEdit.calls.transform).toBe(1);
  expect(afterEdit.code).toContain('dep edited');
});
