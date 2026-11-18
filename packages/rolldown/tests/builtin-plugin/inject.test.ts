import path from 'node:path';
import { injectPlugin } from 'rolldown/plugins';
import { expect, test } from 'vitest';
import type { BindingCallableBuiltinPlugin } from '../../src/binding.cjs';

type CallableInjectPlugin = ReturnType<typeof injectPlugin> & BindingCallableBuiltinPlugin;

const inject = (...args: Parameters<typeof injectPlugin>) =>
  injectPlugin(...args) as CallableInjectPlugin;

test('can be called as a transform hook', async () => {
  const plugin = inject({ useKey: ['#app', 'useKey'] }, { exclude: /node_modules/ });
  const code = 'export const key = useKey();';

  const injected = await plugin.transform(code, '/src/main.js', { moduleType: 'js' });
  expect(injected?.code).toContain('import { useKey } from "#app";');

  const excluded = await plugin.transform(code, '/node_modules/dep/index.js', { moduleType: 'js' });
  expect(excluded).toBeNull();
});

test('resolves a relative `include` against `process.cwd()`', async () => {
  const plugin = inject({ useKey: ['#app', 'useKey'] }, { include: 'src/**' });
  const code = 'export const key = useKey();';

  const included = await plugin.transform(code, path.join(process.cwd(), 'src/main.js'), {
    moduleType: 'js',
  });
  expect(included?.code).toContain('import { useKey } from "#app";');

  const other = await plugin.transform(code, path.join(process.cwd(), 'lib/main.js'), {
    moduleType: 'js',
  });
  expect(other).toBeNull();
});

test('finds names written with escapes or next to non-ASCII whitespace', async () => {
  const plugin = inject({ useKey: ['#app', 'useKey'] });

  for (const code of ['useK\\u0065y();', 'useKey ();', '﻿useKey();']) {
    const result = await plugin.transform(code, '/src/main.js', { moduleType: 'js' });
    expect(result?.code).toContain('import { useKey } from "#app";');
  }
});

test('skips names used only as types', async () => {
  const plugin = inject({ Foo: ['#types', 'Foo'] });

  const typeOnly = await plugin.transform('export let foo: Foo;', '/src/main.ts', {
    moduleType: 'ts',
  });
  expect(typeOnly).toBeNull();

  const value = await plugin.transform('export let foo: Foo = new Foo();', '/src/main.ts', {
    moduleType: 'ts',
  });
  expect(value?.code).toContain('import { Foo } from "#types";');
});

test('skips CommonJS modules', async () => {
  const plugin = inject({ useKey: ['#app', 'useKey'] });
  const code = 'module.exports = useKey();';

  for (const id of ['/src/main.cjs', '/src/main.cts?raw']) {
    const result = await plugin.transform(code, id, {
      moduleType: id.includes('.cts') ? 'ts' : 'js',
    });
    expect(result).toBeNull();
  }
});

test('leaves invalid code to Rolldown', async () => {
  const plugin = inject({ useKey: ['#app', 'useKey'] });

  const result = await plugin.transform('var n = 010; useKey();', '/src/main.js', {
    moduleType: 'js',
  });
  expect(result).toBeNull();
});
