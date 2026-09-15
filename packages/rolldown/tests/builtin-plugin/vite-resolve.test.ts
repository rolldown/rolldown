import nodePath from 'node:path';
import { viteResolvePlugin } from 'rolldown/experimental';
import { expect, test } from 'vitest';

// `@rolldown/test-dual-format` maps `import` to `index.mjs` and `require` to
// `index.cjs` in its `exports` field.
const DUAL_FORMAT_PACKAGE = '@rolldown/test-dual-format';

const importer = nodePath.join(import.meta.dirname, 'main.js');

// `fixtures/tsconfig-paths/tsconfig.json` maps `@/*` onto `./src/*`.
const TSCONFIG_PATHS_FIXTURE = nodePath.join(import.meta.dirname, 'fixtures/tsconfig-paths');
const tsconfigPathsImporter = nodePath.join(TSCONFIG_PATHS_FIXTURE, 'src/main.ts');

type CallableResolveIdHook = (
  id: string,
  importer?: string | null,
  options?: {
    isEntry?: boolean;
    kind?: string;
    scan?: boolean;
    custom?: Record<string, unknown>;
  },
) => Promise<{ id: string; packageJsonPath?: string } | null | undefined>;

function createResolveIdHook(
  overrides: { root?: string; tsconfigPaths?: boolean } = {},
): CallableResolveIdHook {
  const plugin = viteResolvePlugin({
    resolveOptions: {
      isBuild: false,
      isProduction: false,
      asSrc: false,
      preferRelative: false,
      root: overrides.root ?? import.meta.dirname,
      scan: false,
      mainFields: ['main'],
      conditions: ['node'],
      externalConditions: ['node'],
      extensions: ['.js'],
      tryIndex: true,
      preserveSymlinks: false,
      tsconfigPaths: overrides.tsconfigPaths ?? false,
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

test('resolveId reports the `packageJsonPath` the native resolver found', async () => {
  const resolveId = createResolveIdHook();

  const resolved = await resolveId(DUAL_FORMAT_PACKAGE, importer, {});
  // Without it the caller has to re-infer the module format from the id alone.
  expect(resolved?.packageJsonPath).toMatch(/test-dual-format[\\/]package\.json$/);
});

test('resolveId only takes the glob path for `custom["vite:import-glob"]`', async () => {
  const resolveId = createResolveIdHook({
    root: TSCONFIG_PATHS_FIXTURE,
    tsconfigPaths: true,
  });

  // A glob pattern cannot be resolved to a file on disk, so an `import.meta.glob`
  // resolution takes the tsconfig `paths` mapping as-is.
  const glob = await resolveId('@/dir/a', tsconfigPathsImporter, {
    custom: { 'vite:import-glob': {} },
  });
  expect(glob?.id).toMatch(/[\\/]src[\\/]dir[\\/]a$/);

  // Everything else goes through the resolver, which appends the extension.
  const plain = await resolveId('@/dir/a', tsconfigPathsImporter, {});
  expect(plain?.id).toMatch(/[\\/]src[\\/]dir[\\/]a\.js$/);

  // `custom` metadata belonging to another plugin must not switch a normal
  // resolution onto the glob path.
  const unrelatedCustom = await resolveId('@/dir/a', tsconfigPathsImporter, {
    custom: { 'some-other-plugin': { key: 'value' } },
  });
  expect(unrelatedCustom?.id).toBe(plain?.id);
});
