import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import nodePath from 'node:path';
import { tmpdir } from 'node:os';
import { rolldown, type Plugin } from 'rolldown';
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
) => Promise<
  | {
      id: string;
      external?: boolean | 'absolute' | 'relative';
      moduleSideEffects?: boolean | 'no-treeshake';
    }
  | null
  | undefined
>;

function createViteResolvePlugin({
  isBuild = false,
  isBundled,
  environmentConsumer = 'server',
  builtins = [],
  convertExternalRequire = false,
  external = true,
  noExternal = true,
  root = import.meta.dirname,
  onWarn,
}: {
  isBuild?: boolean;
  isBundled?: boolean;
  environmentConsumer?: 'client' | 'server';
  builtins?: Array<string | RegExp>;
  convertExternalRequire?: boolean;
  external?: true | string[];
  noExternal?: true | string[];
  root?: string;
  onWarn?: (message: string) => void;
} = {}) {
  return viteResolvePlugin({
    resolveOptions: {
      isBuild,
      isBundled,
      isProduction: false,
      asSrc: false,
      preferRelative: false,
      root,
      scan: false,
      mainFields: ['main'],
      conditions: ['node'],
      externalConditions: ['node'],
      extensions: ['.js'],
      tryIndex: true,
      preserveSymlinks: false,
      tsconfigPaths: false,
    },
    environmentConsumer,
    environmentName: environmentConsumer === 'client' ? 'client' : 'ssr',
    convertExternalRequire,
    builtins,
    external,
    noExternal,
    dedupe: [],
    legacyInconsistentCjsInterop: false,
    resolveSubpathImports: () => undefined,
    onWarn,
  });
}

function createResolveIdHook(
  options: Parameters<typeof createViteResolvePlugin>[0] = {},
): CallableResolveIdHook {
  const plugin = createViteResolvePlugin(options);
  // `viteResolvePlugin` returns a callable builtin plugin, but its declared
  // type does not carry the hook methods.
  return (plugin as unknown as { resolveId: CallableResolveIdHook }).resolveId;
}

async function bundleNodeBuiltinRequire({
  platform,
  builtins = [],
  specifier = 'node:util',
  root,
}: {
  platform: 'browser' | 'node';
  builtins?: Array<string | RegExp>;
  specifier?: string;
  root?: string;
}) {
  const warnings: string[] = [];
  const entryPlugin: Plugin = {
    name: 'entry',
    resolveId(id) {
      if (id === 'entry') return id;
    },
    load(id) {
      if (id === 'entry') return `module.exports = require(${JSON.stringify(specifier)})`;
    },
  };
  const bundle = await rolldown({
    input: 'entry',
    platform,
    plugins: [
      entryPlugin,
      createViteResolvePlugin({
        isBuild: true,
        builtins,
        convertExternalRequire: platform === 'browser',
        external: [],
        root,
        onWarn(message) {
          warnings.push(message);
        },
      }),
    ],
  });
  const { output } = await bundle.generate({ format: 'esm' });
  const chunk = output.find((item) => item.type === 'chunk');
  return { code: chunk!.code, warnings };
}

async function bundleNodeBuiltinNamedImport(options: { isBuild: boolean; isBundled?: boolean }) {
  const entryPlugin: Plugin = {
    name: 'entry',
    resolveId(id) {
      if (id === 'entry') return id;
    },
    load(id) {
      if (id === 'entry') {
        return `import { readFileSync } from 'node:fs'; export const read = () => readFileSync;`;
      }
    },
  };
  const bundle = await rolldown({
    input: 'entry',
    platform: 'browser',
    plugins: [
      entryPlugin,
      createViteResolvePlugin({ ...options, environmentConsumer: 'client', noExternal: [] }),
    ],
  });
  const { output } = await bundle.generate({ format: 'esm' });
  const { read }: { read: () => unknown } = await import(
    `data:text/javascript,${encodeURIComponent(output[0].code)}`
  );
  return read;
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

test('resolveId handles Node builtins when called directly in build mode', async () => {
  const resolveId = createResolveIdHook({
    isBuild: true,
    builtins: ['node:util'],
    convertExternalRequire: true,
  });

  await expect(
    resolveId('node:util', importer, { kind: 'import-statement' }),
  ).resolves.toMatchObject({
    id: 'node:util',
    external: true,
    moduleSideEffects: false,
  });
});

test('converts automatically externalized Node builtin require calls for ssr.target=webworker builds', async () => {
  const { code, warnings } = await bundleNodeBuiltinRequire({ platform: 'browser' });

  expect(code).toContain('from "node:util"');
  expect(code).not.toContain('__require("node:util")');
  expect(warnings).toStrictEqual([
    expect.stringContaining('Automatically externalized node built-in module "node:util"'),
  ]);
});

test('converts configured Node builtin require calls for ssr.target=webworker builds', async () => {
  const { code, warnings } = await bundleNodeBuiltinRequire({
    platform: 'browser',
    builtins: ['node:util'],
  });

  expect(code).toContain('from "node:util"');
  expect(code).not.toContain('__require("node:util")');
  expect(warnings).toStrictEqual([]);
});

test('converts external Node builtin require calls when an npm package with the same name is installed', async () => {
  const root = await mkdtemp(nodePath.join(tmpdir(), 'rolldown-vite-resolve-'));
  try {
    const packageRoot = nodePath.join(root, 'node_modules/util');
    await mkdir(packageRoot, { recursive: true });
    await Promise.all([
      writeFile(nodePath.join(packageRoot, 'index.js'), 'module.exports = {};'),
      writeFile(
        nodePath.join(packageRoot, 'package.json'),
        JSON.stringify({ name: 'util', version: '0.12.5', main: 'index.js' }),
      ),
    ]);

    const { code, warnings } = await bundleNodeBuiltinRequire({
      platform: 'browser',
      builtins: ['util'],
      specifier: 'util',
      root,
    });

    expect(code).toContain('from "util"');
    expect(code).not.toContain('__require("util")');
    expect(warnings).toStrictEqual([]);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('keeps Node builtin require calls for node builds', async () => {
  const { code } = await bundleNodeBuiltinRequire({
    platform: 'node',
    builtins: ['node:util'],
  });
  expect(code).toContain('__require("node:util")');
});

test('a bundled dev environment allows named imports from the Node builtin stub', async () => {
  const read = await bundleNodeBuiltinNamedImport({ isBuild: false, isBundled: true });
  expect(read).toThrow('Cannot access "node:fs.readFileSync" in client code');
});

test('`isBundled` defaults to `isBuild`', async () => {
  const read = await bundleNodeBuiltinNamedImport({ isBuild: true });
  expect(read).toThrow('Cannot access "node:fs.readFileSync" in client code');

  await expect(bundleNodeBuiltinNamedImport({ isBuild: false })).rejects.toThrow('MISSING_EXPORT');
});

test('an optional peer dep id is not externalized when lazy compilation resolves it again', async () => {
  const root = await mkdtemp(nodePath.join(tmpdir(), 'rolldown-vite-resolve-'));
  try {
    const packageRoot = nodePath.join(root, 'node_modules/lib');
    await mkdir(packageRoot, { recursive: true });
    await Promise.all([
      writeFile(
        nodePath.join(packageRoot, 'index.js'),
        `export const load = () => import('foobar');`,
      ),
      writeFile(
        nodePath.join(packageRoot, 'package.json'),
        JSON.stringify({
          name: 'lib',
          version: '1.0.0',
          main: 'index.js',
          peerDependencies: { foobar: '*' },
          peerDependenciesMeta: { foobar: { optional: true } },
        }),
      ),
    ]);
    const resolveId = createResolveIdHook({ root, environmentConsumer: 'client' });

    const resolved = await resolveId('foobar', nodePath.join(packageRoot, 'index.js'), {
      kind: 'dynamic-import',
    });
    expect(resolved).toMatchObject({ id: '__vite-optional-peer-dep:foobar:lib' });

    // Lazy compilation imports the resolved id from a proxy module and resolves it again.
    const resolvedAgain = await resolveId(resolved!.id, `${resolved!.id}?rolldown-lazy=1`, {
      kind: 'dynamic-import',
    });
    expect(resolvedAgain?.id).toBe(resolved!.id);
    expect(resolvedAgain?.external).toBeFalsy();
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
