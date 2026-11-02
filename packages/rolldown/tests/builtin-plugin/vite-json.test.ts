import { viteJsonPlugin } from 'rolldown/experimental';
import { isWasiTest } from 'rolldown-tests/utils';
import { expect, test } from 'vitest';
import type { BindingCallableBuiltinPlugin } from '../../src/binding.cjs';

type CallableJsonPlugin = ReturnType<typeof viteJsonPlugin> & BindingCallableBuiltinPlugin;

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

test('transform parses JSON into an ES module', async () => {
  const plugin = viteJsonPlugin({
    minify: false,
    namedExports: true,
    stringify: false,
  }) as CallableJsonPlugin;

  const result = await plugin.transform('{ "answer": 42 }', '/virtual/answer.json', {
    moduleType: 'json',
  });
  expect(result?.code).toContain('42');
});

test('transform attaches the module id to a plugin error', async () => {
  const plugin = viteJsonPlugin({
    minify: false,
    namedExports: true,
    stringify: false,
  }) as CallableJsonPlugin;

  await expect(
    plugin.transform('{ not json }', '/virtual/broken.json', { moduleType: 'json' }),
  ).rejects.toMatchObject({
    plugin: 'builtin:vite-json',
    hook: 'transform',
    id: '/virtual/broken.json',
  });
});
