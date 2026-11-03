import { execa } from 'execa';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { rolldown } from 'rolldown';
import { viteJsonPlugin } from 'rolldown/experimental';
import { minify } from 'terser';
import { expect, test } from 'vitest';
import { getBindingPath } from '../src/load-binding';
import type { BindingCallableBuiltinPlugin } from '../../src/binding.cjs';

type CallableJsonPlugin = ReturnType<typeof viteJsonPlugin> & BindingCallableBuiltinPlugin;

test.each(['inherited', 'non-enumerable', 'accessor', 'frozen'])(
  'callable plugins preserve %s options',
  async (kind) => {
    const options = kind === 'inherited' ? Object.create({ stringify: true }) : {};
    if (kind === 'non-enumerable' || kind === 'frozen') {
      Object.defineProperty(options, 'stringify', { value: true });
    } else if (kind === 'accessor') {
      Object.defineProperty(options, 'stringify', {
        get() {
          expect(this).toBe(options);
          return true;
        },
      });
    }
    if (kind === 'frozen') Object.freeze(options);
    const plugin = viteJsonPlugin(options) as CallableJsonPlugin;
    const result = await plugin.transform('{"x":1}', 'data.json', { moduleType: 'json' });
    expect(result?.code).toContain('JSON.parse');
  },
);

test('callable plugins do not read unused options', () => {
  expect(() =>
    viteJsonPlugin({
      get unused() {
        throw new Error('unused getter read');
      },
    } as Parameters<typeof viteJsonPlugin>[0]),
  ).not.toThrow();
});

async function bundleHelper(mock: boolean) {
  const bindingPath = fileURLToPath(new URL('../../src/binding.cjs', import.meta.url));
  const utilsPath = fileURLToPath(new URL('../../src/builtin-plugin/utils.ts', import.meta.url));
  const constructorsPath = fileURLToPath(
    new URL('../../src/builtin-plugin/constructors.ts', import.meta.url),
  );
  const bindingCode = mock
    ? await readFile(new URL('./callable-builtin-mock.mjs', import.meta.url), 'utf8')
    : `import { createRequire } from 'node:module';
       export const { BindingCallableBuiltinPlugin } = createRequire(import.meta.url)(${JSON.stringify(getBindingPath())});`;
  const entry = mock
    ? `export { BuiltinPlugin, makeBuiltinPluginCallable } from ${JSON.stringify(utilsPath)};
       export { nativeRoots, setPending } from ${JSON.stringify(bindingPath)};`
    : `export { viteResolvePlugin } from ${JSON.stringify(constructorsPath)};`;
  const bundle = await rolldown({
    input: 'test-entry',
    platform: 'node',
    plugins: [
      {
        name: 'test-callable-binding',
        resolveId(id) {
          if (id === 'test-entry') return '\0test-entry';
          if (id === bindingPath || id === '../binding.cjs') return '\0test-binding.mjs';
        },
        load(id) {
          if (id === '\0test-entry') return entry;
          if (id === '\0test-binding.mjs') return bindingCode;
        },
      },
    ],
  });
  try {
    const { output } = await bundle.generate({ format: 'esm' });
    return output[0].code;
  } finally {
    await bundle.close();
  }
}

test.each([
  { mock: false, minified: false },
  { mock: false, minified: true },
  { mock: true, minified: false },
  { mock: true, minified: true },
])('callback ownership with mock=$mock and minification=$minified', async ({ mock, minified }) => {
  const directory = await mkdtemp(path.join(tmpdir(), 'rolldown-callable-'));
  try {
    let code = await bundleHelper(mock);
    if (minified) code = (await minify(code, { module: true, compress: { passes: 3 } })).code!;
    const file = path.join(directory, `${mock ? 'mock' : 'native'}.mjs`);
    await writeFile(file, code);
    const runner = mock ? 'callable-builtin-ownership.mjs' : 'callable-builtin-gc.mjs';
    await execa(
      process.execPath,
      ['--expose-gc', fileURLToPath(new URL(runner, import.meta.url)), pathToFileURL(file).href],
      { timeout: 15000 },
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
