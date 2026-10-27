import { viteJsonPlugin } from 'rolldown/experimental';
import { expect, test } from 'vitest';
import type { BindingCallableBuiltinPlugin } from '../../src/binding.cjs';

type CallableJsonPlugin = ReturnType<typeof viteJsonPlugin> & BindingCallableBuiltinPlugin;

test('a native hook error gets the caller frames', async () => {
  const plugin = viteJsonPlugin({}) as CallableJsonPlugin;

  async function callTransform() {
    return await plugin.transform('{"x":', 'data.json', { moduleType: 'json' });
  }
  const error = await callTransform().then(
    () => undefined,
    (e: Error) => e,
  );
  expect(error?.message).toContain('EOF while parsing');
  // The native error has no frames. The hook wrapper captures them and trims its own frame.
  const frames = error?.stack?.split('\n').filter((line) => /^\s+at /.test(line)) ?? [];
  expect(frames[0]).toContain('callTransform');
  expect(error?.stack).not.toContain('invokeBuiltinHook');
});
