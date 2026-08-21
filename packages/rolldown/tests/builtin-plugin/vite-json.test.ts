import { viteJsonPlugin } from 'rolldown/experimental';
import { expect, test } from 'vitest';

type CallableTransformHook = (
  code: string,
  id: string,
  options: { moduleType: string },
) => Promise<{ code?: string } | null | undefined>;

function createTransformHook(): CallableTransformHook {
  const plugin = viteJsonPlugin({ minify: false, namedExports: true, stringify: false });
  // `viteJsonPlugin` returns a callable builtin plugin, but its declared type
  // does not carry the hook methods.
  return (plugin as unknown as { transform: CallableTransformHook }).transform;
}

test('transform parses JSON into an ES module', async () => {
  const transform = createTransformHook();

  const result = await transform('{ "answer": 42 }', '/virtual/answer.json', {
    moduleType: 'json',
  });
  expect(result?.code).toContain('42');
});

test('transform attaches the module id to a plugin error', async () => {
  const transform = createTransformHook();

  await expect(
    transform('{ not json }', '/virtual/broken.json', { moduleType: 'json' }),
  ).rejects.toMatchObject({
    plugin: 'builtin:vite-json',
    hook: 'transform',
    id: '/virtual/broken.json',
  });
});
