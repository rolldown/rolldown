import assert from 'node:assert/strict';
import { copyFile, cp, mkdtemp, readFile, readdir, rename, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { parse } from 'acorn';

const defaultRepoRoot = fileURLToPath(new URL('../../', import.meta.url));
const coreRuntimePackages = ['@emnapi/core', '@emnapi/runtime', '@napi-rs/wasm-runtime'];
const runtimePackages = [...coreRuntimePackages, 'buffer'];
const notices = ['LICENSE', 'THIRD-PARTY-LICENSE'];

async function assertEmbeddedRuntimeNotices(repoRoot, runtimeFsBundle) {
  const code = await readFile(runtimeFsBundle, 'utf8');
  const sourceMapMarker = '//# sourceMappingURL=data:application/json;charset=utf-8;base64,';
  const sourceMapIndex = code.lastIndexOf(sourceMapMarker);
  assert.notEqual(sourceMapIndex, -1, 'wasm-runtime filesystem bundle must include its source map');
  const encodedSourceMap = code.slice(sourceMapIndex + sourceMapMarker.length).split(/\r?\n/, 1)[0];
  const sourceMap = JSON.parse(Buffer.from(encodedSourceMap, 'base64').toString('utf8'));
  const embeddedPackages = [
    ...new Set(
      sourceMap.sources
        .map(
          (source) =>
            source.match(
              /node_modules\/(?:\.pnpm\/[^/]+\/node_modules\/)?((?:@[^/]+\/)?[^/]+)/,
            )?.[1],
        )
        .filter(Boolean),
    ),
  ].sort((a, b) => a.localeCompare(b));
  const thirdPartyLicense = await readFile(path.join(repoRoot, 'THIRD-PARTY-LICENSE'), 'utf8');

  for (const packageName of embeddedPackages) {
    assert.ok(
      thirdPartyLicense.includes(`  - ${packageName}\n`),
      `THIRD-PARTY-LICENSE must inventory embedded wasm-runtime package ${packageName}`,
    );
  }
}

function isBareRuntimeSpecifier(specifier) {
  return /^(?:@(?:emnapi|napi-rs)\/|(?:node:)?buffer$)/.test(specifier);
}

function findBareRuntimeImports(code, sourceType) {
  const program = parse(code, { ecmaVersion: 'latest', sourceType });
  const imports = [];
  const pending = [program];

  while (pending.length > 0) {
    const node = pending.pop();
    if (!node || typeof node !== 'object') continue;

    if (
      (node.type === 'ImportDeclaration' ||
        node.type === 'ExportNamedDeclaration' ||
        node.type === 'ExportAllDeclaration') &&
      typeof node.source?.value === 'string' &&
      isBareRuntimeSpecifier(node.source.value)
    ) {
      imports.push(node.source.value);
    }
    if (
      node.type === 'ImportExpression' &&
      typeof node.source?.value === 'string' &&
      isBareRuntimeSpecifier(node.source.value)
    ) {
      imports.push(node.source.value);
    }
    if (
      node.type === 'CallExpression' &&
      node.arguments?.length === 1 &&
      typeof node.arguments[0]?.value === 'string' &&
      isBareRuntimeSpecifier(node.arguments[0].value) &&
      ((node.callee?.type === 'Identifier' && node.callee.name === 'require') ||
        (node.callee?.type === 'MemberExpression' &&
          node.callee.object?.type === 'Identifier' &&
          node.callee.object.name === 'require' &&
          node.callee.property?.type === 'Identifier' &&
          node.callee.property.name === 'resolve'))
    ) {
      imports.push(node.arguments[0].value);
    }

    for (const value of Object.values(node)) {
      if (Array.isArray(value)) {
        pending.push(...value);
      } else if (value && typeof value === 'object') {
        pending.push(value);
      }
    }
  }

  return imports.sort((a, b) => a.localeCompare(b));
}

assert.deepEqual(
  findBareRuntimeImports(
    "import('@emnapi/core'); require.resolve('@napi-rs/wasm-runtime'); export { Buffer } from 'node:buffer'; require('buffer');",
    'module',
  ),
  ['@emnapi/core', '@napi-rs/wasm-runtime', 'buffer', 'node:buffer'],
  'runtime import scan must cover re-exports, dynamic imports, require, and require.resolve',
);

function assertHardenedRuntime(code, loader) {
  const callbackResultWrites =
    code.match(
      /v = envObject\.ensureHandleId\(ret\);\s*new DataView\(wasmMemory\.buffer\)\.setUint32\(result, v, true\)/g,
    ) ?? [];
  assert.ok(
    callbackResultWrites.length >= 2,
    `${loader} does not contain hardened napi_call_function/napi_new_instance result writes`,
  );
  assert.match(
    code,
    /var state = function\(\) \{\s*return new Int32Array\(emnapiTSFN\.ensureBufferFor\(end\)\);\s*\}/,
  );
  assert.match(code, /Atomics\.exchange\(state\(\), scheduled >>> 2, 1\)/);
  assert.match(
    code,
    /function getThreadSpawnResultView\(memory, address, wasm64\)/,
    `${loader} does not contain the shared-memory thread-spawn refresh helper`,
  );
  assert.match(code, /address \+ THREAD_SPAWN_RESULT_SIZE > buffer\.byteLength/);
  assert.match(code, /memory\.grow\(BigInt\(0\)\)/);
  assert.match(code, /memory\.grow\(0\)/);
  assert.ok(
    (code.match(/getThreadSpawnResultView\(/g) ?? []).length >= 3,
    `${loader} does not refresh both wasi-threads thread-spawn result writes`,
  );
}

export async function replaceDirectoriesTransactionally(replacements, { afterOperation } = {}) {
  if (replacements.length === 0) return;

  const backupRoot = await mkdtemp(
    path.join(path.dirname(replacements[0].destination), '.stage-wasi-backup-'),
  );
  const states = replacements.map((replacement, index) => ({
    ...replacement,
    backup: path.join(backupRoot, String(index)),
    originalMoved: false,
    stagedMoved: false,
  }));
  let preserveBackupRoot = false;

  try {
    for (const [index, state] of states.entries()) {
      await rename(state.destination, state.backup);
      state.originalMoved = true;
      await afterOperation?.('backup', index);
      await rename(state.staged, state.destination);
      state.stagedMoved = true;
      await afterOperation?.('install', index);
    }
  } catch (error) {
    const rollbackErrors = [];
    for (const state of states.toReversed()) {
      try {
        if (state.stagedMoved) {
          await rename(state.destination, state.staged);
          state.stagedMoved = false;
        }
        if (state.originalMoved) {
          await rename(state.backup, state.destination);
          state.originalMoved = false;
        }
      } catch (rollbackError) {
        rollbackErrors.push(rollbackError);
      }
    }
    if (rollbackErrors.length > 0) {
      preserveBackupRoot = true;
      throw new AggregateError(
        [error, ...rollbackErrors],
        `WASI package transaction failed and rollback was incomplete; backups are preserved at ${backupRoot}`,
      );
    }
    throw error;
  } finally {
    if (!preserveBackupRoot) {
      await rm(backupRoot, { force: true, recursive: true });
    }
  }
}

export async function stageWasiPackages({ repoRoot = defaultRepoRoot, transactionHook } = {}) {
  const publicTypeDependencies = {
    '@oxc-project/types': JSON.parse(
      await readFile(
        path.join(repoRoot, 'packages/rolldown/node_modules/@oxc-project/types/package.json'),
        'utf8',
      ),
    ).version,
  };
  const runtimeFsBundle = path.join(
    repoRoot,
    'packages/rolldown/node_modules/@napi-rs/wasm-runtime/dist/fs.js',
  );
  const flavors = [
    {
      label: 'threaded',
      generatedRuntimePackages: coreRuntimePackages,
      declaration: path.join(repoRoot, 'packages/rolldown/src/rolldown-binding.wasi.d.cts'),
      sourceDir: path.join(repoRoot, 'packages/rolldown/artifacts/threaded-wasi-loaders'),
      packageDir: path.join(repoRoot, 'packages/rolldown/npm/wasm32-wasi'),
      exactSourceSet: true,
      loaders: [
        { name: 'rolldown-binding.wasi.cjs', sourceType: 'script' },
        { name: 'rolldown-binding.wasi-browser.js', sourceType: 'module' },
        { name: 'wasi-worker.mjs', sourceType: 'module' },
        { name: 'wasi-worker-browser.mjs', sourceType: 'module' },
      ],
      packFiles: [
        'rolldown-binding.wasm32-wasi.wasm',
        'rolldown-binding.wasi.cjs',
        'rolldown-binding.wasi.d.cts',
        'rolldown-binding.wasi-browser.js',
        'wasi-worker.mjs',
        'wasi-worker-browser.mjs',
        ...notices,
      ],
    },
    {
      label: 'threadless',
      generatedRuntimePackages: runtimePackages,
      declaration: path.join(repoRoot, 'packages/rolldown/src/rolldown-binding.wasip1.d.cts'),
      sourceDir: path.join(repoRoot, 'packages/browser/dist'),
      packageDir: path.join(repoRoot, 'packages/rolldown/npm/wasm32-wasip1'),
      exactSourceSet: false,
      loaders: [
        { name: 'rolldown-binding.wasip1.cjs', sourceType: 'script' },
        { name: 'rolldown-binding.wasip1-browser.js', sourceType: 'module' },
        {
          name: 'rolldown-binding.wasip1-deferred.js',
          sourceName: 'workerd.browser.mjs',
          sourceType: 'module',
        },
      ],
      packFiles: [
        'rolldown-binding.wasm32-wasip1.wasm',
        'rolldown-binding.wasip1.cjs',
        'rolldown-binding.wasip1.d.cts',
        'rolldown-binding.wasip1-browser.js',
        'rolldown-binding.wasip1-deferred.js',
        'rolldown-binding.wasip1-deferred.d.ts',
        'rolldown-binding.wasm32-wasip1.wasm.d.ts',
        ...notices,
      ],
    },
  ];

  await assertEmbeddedRuntimeNotices(repoRoot, runtimeFsBundle);

  const stagingRoot = await mkdtemp(
    path.join(repoRoot, 'packages/rolldown/npm/.stage-wasi-packages-'),
  );
  try {
    for (const flavor of flavors) {
      const {
        label,
        generatedRuntimePackages,
        declaration,
        sourceDir,
        packageDir,
        exactSourceSet,
        loaders,
        packFiles,
      } = flavor;
      const stagedPackageDir = path.join(stagingRoot, label);
      flavor.stagedPackageDir = stagedPackageDir;
      await cp(packageDir, stagedPackageDir, { recursive: true });

      const loaderNames = loaders.map(({ name }) => name).sort();
      if (exactSourceSet) {
        assert.deepEqual(
          (await readdir(sourceDir)).sort(),
          loaderNames,
          `${label} WASI loader artifact must contain exactly its runtime loader graph`,
        );
      }

      await copyFile(declaration, path.join(stagedPackageDir, path.basename(declaration)));

      for (const { name, sourceName = name, sourceType } of loaders) {
        const source = path.join(sourceDir, sourceName);
        const destination = path.join(stagedPackageDir, name);
        await copyFile(source, destination);
        const code = await readFile(destination, 'utf8');
        assert.deepEqual(
          findBareRuntimeImports(code, sourceType),
          [],
          `${name} must vendor its Buffer/emnapi/wasm runtime`,
        );
        assertHardenedRuntime(code, name);
      }

      if (label === 'threadless') {
        await copyFile(
          path.join(sourceDir, 'workerd.d.mts'),
          path.join(stagedPackageDir, 'rolldown-binding.wasip1-deferred.d.ts'),
        );
        const managedWorkerd = await readFile(
          path.join(stagedPackageDir, 'rolldown-binding.wasip1-deferred.js'),
          'utf8',
        );
        assert.match(managedWorkerd, /getCurrentThreadTaskHostContractVersion/);
        assert.match(managedWorkerd, /registerCurrentThreadTaskHost/);
        assert.match(managedWorkerd, /unregisterCurrentThreadTaskHost/);
        assert.match(managedWorkerd, /__actualVersion !== 2/);
        assert.match(managedWorkerd, /Reflect\.apply\(__register, __binding, \[\]\)/);
        assert.match(managedWorkerd, /Reflect\.apply\(__unregister, __binding, __registration\)/);
        assert.doesNotMatch(
          managedWorkerd,
          /driveCurrentThreadRuntimeTasks|cancelCurrentThreadRuntimeTaskDispatch|dispatchHigh|dispatchLow/,
        );
        assert.match(managedWorkerd, /registerTimerHost/);
        assert.match(
          managedWorkerd,
          /createInstance\s*=\s*instantiate|instantiate\s*=\s*createInstance/,
        );
        assert.doesNotMatch(managedWorkerd, /from\s+['"]node:/);
      }

      if (label === 'threaded') {
        assert.match(
          await readFile(path.join(stagedPackageDir, 'rolldown-binding.wasi.cjs'), 'utf8'),
          /wasi-worker\.mjs/,
        );
        assert.match(
          await readFile(path.join(stagedPackageDir, 'rolldown-binding.wasi-browser.js'), 'utf8'),
          /wasi-worker-browser\.mjs/,
        );
      }

      for (const notice of notices) {
        await copyFile(path.join(repoRoot, notice), path.join(stagedPackageDir, notice));
      }

      const manifestPath = path.join(stagedPackageDir, 'package.json');
      const manifest = JSON.parse(await readFile(manifestPath, 'utf8'));
      assert.deepEqual(
        (manifest.files ?? [])
          .filter((file) => !notices.includes(file))
          .sort((a, b) => a.localeCompare(b)),
        packFiles.filter((file) => !notices.includes(file)).sort((a, b) => a.localeCompare(b)),
        `${manifest.name} generated packlist drifted from its complete ${label} artifact set`,
      );
      const declaredRuntimePackages = runtimePackages.filter(
        (dependency) => manifest.dependencies?.[dependency],
      );
      if (declaredRuntimePackages.length > 0) {
        assert.deepEqual(
          declaredRuntimePackages,
          generatedRuntimePackages,
          `${manifest.name} has an incomplete generated runtime dependency set`,
        );
      }
      for (const dependency of runtimePackages) {
        delete manifest.dependencies[dependency];
      }
      manifest.dependencies = {
        ...manifest.dependencies,
        ...publicTypeDependencies,
      };
      if (manifest.dependencies && Object.keys(manifest.dependencies).length === 0) {
        delete manifest.dependencies;
      }
      manifest.files = packFiles;
      await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
      assert.deepEqual(
        (await readdir(stagedPackageDir)).sort((a, b) => a.localeCompare(b)),
        [...packFiles, 'README.md', 'package.json'].sort((a, b) => a.localeCompare(b)),
        `${manifest.name} package directory must equal its declared artifact set`,
      );
    }

    await replaceDirectoriesTransactionally(
      flavors.map(({ packageDir, stagedPackageDir }) => ({
        destination: packageDir,
        staged: stagedPackageDir,
      })),
      { afterOperation: transactionHook },
    );
  } finally {
    await rm(stagingRoot, { force: true, recursive: true });
  }

  console.log(
    `Staged self-contained WASI loaders in ${flavors
      .map(({ packageDir }) => path.relative(repoRoot, packageDir))
      .join(' and ')}`,
  );
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  await stageWasiPackages();
}
