import nodePath from 'node:path';
import { viteResolvePlugin } from 'rolldown/experimental';
import { expect, test } from 'vitest';

// `@rolldown/test-dual-format` maps `import` to `index.mjs` and `require` to
// `index.cjs` in its `exports` field.
const DUAL_FORMAT_PACKAGE = '@rolldown/test-dual-format';

const importer = nodePath.join(import.meta.dirname, 'main.js');

type CallableResolveIdHook = (
  id: string,
  importer?: string | null,
  options?: { isEntry?: boolean; kind?: string; scan?: boolean },
) => Promise<{ id: string } | null | undefined>;

function createResolveIdHook(): CallableResolveIdHook {
  const plugin = viteResolvePlugin({
    resolveOptions: {
      isBuild: false,
      isProduction: false,
      asSrc: false,
      preferRelative: false,
      root: import.meta.dirname,
      scan: false,
      mainFields: ['main'],
      conditions: ['node'],
      externalConditions: ['node'],
      extensions: ['.js'],
      tryIndex: true,
      preserveSymlinks: false,
      tsconfigPaths: false,
    },
    environmentConsumer: 'server',
    environmentName: 'ssr',
    builtins: [],
    external: true,
    noExternal: true,
    dedupe: [],
    legacyInconsistentCjsInterop: false,
    resolveSubpathImports: () => undefined,
  });
  // `viteResolvePlugin` returns a callable builtin plugin, but its declared
  // type does not carry the hook methods.
  return (plugin as unknown as { resolveId: CallableResolveIdHook }).resolveId;
}

test('resolveId forwards `options.kind` to the native resolver', async () => {
  const resolveId = createResolveIdHook();

  const requireCall = await resolveId(DUAL_FORMAT_PACKAGE, importer, {
    kind: 'require-call',
  });
  expect(requireCall?.id).toMatch(/index\.cjs$/);

  const importStatement = await resolveId(DUAL_FORMAT_PACKAGE, importer, {
    kind: 'import-statement',
  });
  expect(importStatement?.id).toMatch(/index\.mjs$/);
});

test('resolveId defaults to the `import` kind when `options.kind` is absent', async () => {
  const resolveId = createResolveIdHook();

  const resolved = await resolveId(DUAL_FORMAT_PACKAGE, importer, {});
  expect(resolved?.id).toMatch(/index\.mjs$/);
});

test('resolveId rejects an invalid `options.kind`', async () => {
  const resolveId = createResolveIdHook();

  await expect(resolveId(DUAL_FORMAT_PACKAGE, importer, { kind: 'not-a-kind' })).rejects.toThrow(
    'Invalid import kind',
  );
});
