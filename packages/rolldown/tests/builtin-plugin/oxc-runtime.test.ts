import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { oxcRuntimePlugin } from 'rolldown/experimental';
import { parseAst } from 'rolldown/parseAst';
import { expect, test } from 'vitest';

const plugin = oxcRuntimePlugin() as unknown as {
  resolveId: { handler: Function; order: string };
  load: { handler: Function; order: string };
};

const runtimeDir = dirname(
  createRequire(import.meta.url).resolve('@oxc-project/runtime/package.json'),
);

function readRuntimeHelper(name: string, esm: boolean) {
  return readFile(join(runtimeDir, 'src/helpers', esm ? 'esm' : '', `${name}.js`), 'utf8');
}

function expectMinifiedHelper(code: string, original: string) {
  expect(() => parseAst(code)).not.toThrow();
  expect(code.length).toBeLessThan(original.length);
}

test('resolveId resolves oxc runtime helper to virtual module', async () => {
  const result = await plugin.resolveId.handler('@oxc-project/runtime/helpers/objectSpread2.js');
  // Non-`require` callers get routed to the ESM variant under `helpers/esm/`.
  // oxlint-disable-next-line no-control-regex
  expect(result.id).toMatch(/^\0@oxc-project\+runtime@[\d.]+\/helpers\/esm\/objectSpread2\.js$/);
});

test('resolveId returns null for non-matching specifier', async () => {
  const result = await plugin.resolveId.handler('some-random-module');
  expect(result).toBeNull();
});

test('load returns minified ESM and CJS helper code', async () => {
  const resolved = await plugin.resolveId.handler('@oxc-project/runtime/helpers/objectSpread2.js');
  const esmResult = await plugin.load.handler(resolved.id);
  const originalEsm = await readRuntimeHelper('objectSpread2', true);
  expectMinifiedHelper(esmResult.code, originalEsm);

  const cjsId = resolved.id.replace('/helpers/esm/', '/helpers/');
  const cjsResult = await plugin.load.handler(cjsId);
  const originalCjs = await readRuntimeHelper('objectSpread2', false);
  expectMinifiedHelper(cjsResult.code, originalCjs);

  const repeatedResult = await plugin.load.handler(resolved.id);
  expect(repeatedResult.code).toBe(esmResult.code);
});

test('load returns a large helper and rejects an unknown helper', async () => {
  const resolved = await plugin.resolveId.handler(
    '@oxc-project/runtime/helpers/regeneratorRuntime.js',
  );
  const result = await plugin.load.handler(resolved.id);
  const original = await readRuntimeHelper('regeneratorRuntime', true);
  expectMinifiedHelper(result.code, original);

  const unknownId = resolved.id.replace('regeneratorRuntime.js', 'unknown.js');
  expect(await plugin.load.handler(unknownId)).toBeNull();
});

test('has order', async () => {
  expect(plugin.resolveId.order).toBe('pre');
  expect(plugin.load.order).toBe('pre');
});
