import { rolldown, VERSION } from 'rolldown';
import { esmExternalRequirePlugin } from 'rolldown/plugins';
import { expect, test } from 'vitest';

function otherCopyPlugin(version: string, name: string, options?: unknown) {
  class OtherBuiltinPlugin {
    constructor(
      public name: string,
      public _options?: unknown,
    ) {}
  }

  Object.defineProperty(OtherBuiltinPlugin.prototype, Symbol.for('rolldown.BuiltinPlugin'), {
    value: version,
  });

  return new OtherBuiltinPlugin(name, options);
}

const input = 'virtual:entry';
const entryPlugin = {
  name: 'entry',
  resolveId: (id: string) => (id === input ? id : undefined),
  load: (id: string) => (id === input ? 'export default __X__' : undefined),
};

test('throw on a built-in plugin from another rolldown version', async () => {
  const bundle = await rolldown({
    input,
    plugins: [entryPlugin, otherCopyPlugin('1.0.0', 'builtin:vite-dynamic-import-vars')],
  });
  await expect(bundle.generate()).rejects.toThrow(
    `Built-in plugin "builtin:vite-dynamic-import-vars" was created by rolldown 1.0.0, but this build runs on rolldown ${VERSION}.`,
  );
});

test('run a built-in plugin from another copy of the same rolldown version', async () => {
  const plugin = otherCopyPlugin(VERSION, 'builtin:replace', { values: { __X__: '"replaced"' } });
  const bundle = await rolldown({ input, plugins: [entryPlugin, plugin] });
  const { output } = await bundle.generate();
  expect(output[0].code).toContain('"replaced"');
});

test('a built-in plugin carries the brand', () => {
  expect(Reflect.get(esmExternalRequirePlugin(), Symbol.for('rolldown.BuiltinPlugin'))).toBe(
    VERSION,
  );
});
