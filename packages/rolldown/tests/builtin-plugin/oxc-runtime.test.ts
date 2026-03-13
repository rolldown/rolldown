import { oxcRuntimePlugin } from 'rolldown/experimental';
import { expect, test } from 'vitest';

const plugin = oxcRuntimePlugin() as unknown as { resolveId: Function; load: Function };

test('resolveId resolves oxc runtime helper to virtual module', async () => {
  const result = await plugin.resolveId('@oxc-project/runtime/helpers/objectSpread2.js');
  // oxlint-disable-next-line no-control-regex
  expect(result.id).toMatch(/^\0@oxc-project\+runtime@[\d.]+\/helpers\/objectSpread2\.js$/);
});

test('resolveId returns null for non-matching specifier', async () => {
  const result = await plugin.resolveId('some-random-module');
  expect(result).toBeNull();
});

test('load returns code for resolved virtual module', async () => {
  const resolved = await plugin.resolveId('@oxc-project/runtime/helpers/objectSpread2.js');
  const result = await plugin.load(resolved.id);
  expect(result).toBeTruthy();
  expect(result.code).toBeTruthy();
  expect(typeof result.code).toBe('string');
});
