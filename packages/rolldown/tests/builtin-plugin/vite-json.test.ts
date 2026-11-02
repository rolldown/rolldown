import { viteJsonPlugin } from 'rolldown/experimental';
import { isWasiTest } from 'rolldown-tests/utils';
import { expect, test } from 'vitest';
import type { BindingCallableBuiltinPlugin } from '../../src/binding.cjs';

type CallableJsonPlugin = ReturnType<typeof viteJsonPlugin> & BindingCallableBuiltinPlugin;

test.each(
  ['JSON', 'nested', 'invalid-key'].flatMap((key) =>
    [false, true].map((reverse) => ({ key, reverse })),
  ),
)('a JSON export does not shadow JSON.parse ($key, reverse=$reverse)', async ({ key, reverse }) => {
  const nested = { text: 'x'.repeat(12000), ['__proto__']: { polluted: true } };
  const properties = {
    JSON: null,
    JSON$1: 'first',
    JSON$2: 'second',
    globalThis: null,
    [key]: nested,
    ['__proto__']: 42,
  };
  const data = reverse ? Object.fromEntries(Object.entries(properties).reverse()) : properties;
  const plugin = viteJsonPlugin({
    namedExports: true,
    stringify: 'auto',
    minify: false,
  }) as CallableJsonPlugin;
  const result = await plugin.transform(JSON.stringify(data), 'data.json', { moduleType: 'json' });
  expect(result?.code).toContain('JSON.parse(');

  const exports = await import(`data:text/javascript,${encodeURIComponent(result!.code!)}`);
  expect(exports.default).toEqual(data);
  for (const name of ['JSON', 'JSON$1', 'JSON$2', '__proto__']) {
    expect(exports[name]).toBe(exports.default[name]);
  }
  if (key === 'nested') expect(exports.nested).toBe(exports.default.nested);
  expect(Object.getPrototypeOf(exports.default)).toBe(Object.prototype);
  expect(Object.hasOwn(exports.default, '__proto__')).toBe(true);
  expect(Object.getPrototypeOf(exports.default[key])).toBe(Object.prototype);
  expect(Object.hasOwn(exports.default[key], '__proto__')).toBe(true);
});

// With the WASI binding, emnapi creates the error with its own stack frame. The hook wrapper
// keeps that stack. Then the caller frame is not the first frame.
test.skipIf(isWasiTest)('a native hook error gets the caller frames', async () => {
  const plugin = viteJsonPlugin({}) as CallableJsonPlugin;

  async function callTransform() {
    return await plugin.transform('{"x":', 'data.json', { moduleType: 'json' });
  }
  const error = await callTransform().then(
    () => undefined,
    (e: Error) => e,
  );
  expect(error?.message).toContain('EOF while parsing');
  // The native error has no stack frames. The hook wrapper captures the frames and removes
  // its own frame.
  const frames = error?.stack?.split('\n').filter((line) => /^\s+at /.test(line)) ?? [];
  expect(frames[0]).toContain('callTransform');
  expect(error?.stack).not.toContain('invokeBuiltinHook');
});
