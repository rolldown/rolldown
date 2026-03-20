import { oxcRuntimePlugin } from 'rolldown/experimental';
import { expect, test } from 'vitest';

const plugin = oxcRuntimePlugin() as unknown as {
  resolveId: { handler: Function; order: string };
  load: { handler: Function; order: string };
};

test('resolveId resolves oxc runtime helper to virtual module', async () => {
  const result = await plugin.resolveId.handler('@oxc-project/runtime/helpers/objectSpread2.js');
  // oxlint-disable-next-line no-control-regex
  expect(result.id).toMatch(/^\0@oxc-project\+runtime@[\d.]+\/helpers\/objectSpread2\.js$/);
});

test('resolveId returns null for non-matching specifier', async () => {
  const result = await plugin.resolveId.handler('some-random-module');
  expect(result).toBeNull();
});

test('load returns code for resolved virtual module', async () => {
  const resolved = await plugin.resolveId.handler('@oxc-project/runtime/helpers/objectSpread2.js');
  const result = await plugin.load.handler(resolved.id);
  expect(result).toBeTruthy();
  expect(result.code).toBeTruthy();
  expect(typeof result.code).toBe('string');
});

test('has order', async () => {
  expect(plugin.resolveId.order).toBe('pre');
  expect(plugin.load.order).toBe('pre');
});
