import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import {
  chmod,
  copyFile,
  cp,
  lstat,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  realpath,
  rename,
  rm,
  writeFile,
} from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { NapiCli } from '@napi-rs/cli';

import { findBareRuntimeImports } from './bare-runtime-imports.mjs';

const defaultRepoRoot = fileURLToPath(new URL('../../', import.meta.url));
const coreRuntimePackages = ['@emnapi/core', '@emnapi/runtime', '@napi-rs/wasm-runtime'];
const runtimePackages = [...coreRuntimePackages, 'buffer'];
const notices = ['LICENSE', 'THIRD-PARTY-LICENSE'];
const stagingDirectoryPrefix = '.stage-wasi-packages-';
const packageRootDirectoryMode = 0o775;
const napiCli = new NapiCli();

// @napi-rs/wasm-runtime stopped shipping dist/fs.js with an inline source map
// after 1.2.0, so the embedded-package inventory can no longer be derived from
// the bundle itself. Pin each audited map-less bundle by content hash: a
// wasm-runtime bump that changes the bundle must re-audit its embedded packages
// (diff against the last audited bundle, or rebuild it upstream with source
// maps) and update BOTH this table and the THIRD-PARTY-LICENSE inventory.

const auditedRuntimeFsBundles = new Map([
  [
    // dist/fs.js of @napi-rs/wasm-runtime 1.2.3. 1.2.3 rebuilt the bundle with
    // rolldown instead of rollup, so it is not a byte-diff of the 1.2.2/1.2.0
    // bundle and was re-audited from its `//#region ../node_modules/<pkg>/...`
    // markers (every node_modules path in the bundle is covered by one). That
    // derivation reproduces 1.2.0's source-map inventory exactly when replayed
    // on 1.2.0, and yields the same package set here: `memfs` keeps no region
    // marker of its own but is still a bundled devDependency (its API is
    // re-exported through the @jsonjoy.com/fs-* modules), so it stays listed.
    '7357c7859efc35d7b3cbe37d2cec912f12d554403fb6b48e1a7ac035c43cbb70',
    {
      version: '1.2.3',
      packages: [
        '@jsonjoy.com/base64',
        '@jsonjoy.com/buffers',
        '@jsonjoy.com/fs-core',
        '@jsonjoy.com/fs-node',
        '@jsonjoy.com/fs-node-builtins',
        '@jsonjoy.com/fs-node-utils',
        '@jsonjoy.com/fs-print',
        '@jsonjoy.com/fs-snapshot',
        '@jsonjoy.com/json-pack',
        'abort-controller',
        'async-function',
        'async-generator-function',
        'base64-js',
        'buffer',
        'call-bind-apply-helpers',
        'call-bound',
        'dunder-proto',
        'es-define-property',
        'es-errors',
        'es-object-atoms',
        'events',
        'function-bind',
        'generator-function',
        'get-intrinsic',
        'get-proto',
        'glob-to-regex.js',
        'gopd',
        'has-symbols',
        'hasown',
        'ieee754',
        'math-intrinsics',
        'memfs',
        'object-inspect',
        'path-browserify',
        'process',
        'punycode',
        'qs',
        'readable-stream',
        'safe-buffer',
        'side-channel',
        'side-channel-list',
        'side-channel-map',
        'side-channel-weakmap',
        'string_decoder',
        'thingies',
        'tree-dump',
        'tslib',
        'url',
      ],
    },
  ],
]);

async function assertEmbeddedRuntimeNotices(repoRoot, runtimeFsBundle) {
  const code = await readFile(runtimeFsBundle, 'utf8');
  assert.equal(
    code.lastIndexOf('//# sourceMappingURL=data:application/json;charset=utf-8;base64,'),
    -1,
    'wasm-runtime filesystem bundle unexpectedly ships an inline source map; re-derive the embedded package inventory from it and update auditedRuntimeFsBundles',
  );
  const bundleHash = createHash('sha256').update(code).digest('hex');
  const auditedBundle = auditedRuntimeFsBundles.get(bundleHash);
  assert.ok(
    auditedBundle,
    `wasm-runtime filesystem bundle content hash ${bundleHash} is not audited; diff the bundle against the last audited version, then update auditedRuntimeFsBundles and the THIRD-PARTY-LICENSE inventory`,
  );
  const thirdPartyLicense = await readFile(path.join(repoRoot, 'THIRD-PARTY-LICENSE'), 'utf8');

  for (const packageName of auditedBundle.packages) {
    assert.ok(
      thirdPartyLicense.includes(`  - ${packageName}\n`),
      `THIRD-PARTY-LICENSE must inventory embedded wasm-runtime package ${packageName}`,
    );
  }
}

async function assertDirectory(candidate, label) {
  const stats = await lstat(candidate);
  if (!stats.isDirectory()) throw new Error(`${label} is not a directory: ${candidate}`);
}

async function assertRegularFile(candidate, label) {
  const stats = await lstat(candidate);
  if (!stats.isFile()) throw new Error(`${label} is not a regular file: ${candidate}`);
}

function assertOptionalDirectory(stats, candidate, label) {
  if (stats && !stats.isDirectory()) throw new Error(`${label} is not a directory: ${candidate}`);
}

async function lstatIfExists(candidate) {
  try {
    return await lstat(candidate);
  } catch (error) {
    if (isNodeError(error) && error.code === 'ENOENT') return undefined;
    throw error;
  }
}

function isNodeError(error) {
  return error instanceof Error && 'code' in error;
}

export async function stageWasiPackages({ repoRoot = defaultRepoRoot } = {}) {
  const packageRoot = path.join(repoRoot, 'packages/rolldown/npm');
  const rolldownRoot = path.join(repoRoot, 'packages/rolldown');
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
      wasm: path.join(repoRoot, 'packages/rolldown/src/rolldown-binding.wasm32-wasi.wasm'),
      sourceDir: path.join(repoRoot, 'packages/rolldown/artifacts/threaded-wasi-loaders'),
      packageName: 'wasm32-wasi',
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
      wasm: path.join(repoRoot, 'packages/rolldown/src/rolldown-binding.wasm32-wasip1.wasm'),
      sourceDir: path.join(repoRoot, 'packages/browser/dist'),
      packageName: 'wasm32-wasip1',
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

  const createdPackageRoot = await mkdir(packageRoot, {
    mode: packageRootDirectoryMode,
    recursive: true,
  });
  if (createdPackageRoot && process.platform !== 'win32') {
    await chmod(packageRoot, packageRootDirectoryMode);
  }
  await assertDirectory(packageRoot, 'WASI package root');
  const canonicalPackageRoot = await realpath(packageRoot);

  await removeOrphanedStagingDirectories(canonicalPackageRoot);
  await ensureWasiPackageDirectories({
    packageNames: flavors.map(({ packageName }) => packageName),
    packageRoot: canonicalPackageRoot,
    rolldownRoot,
  });
  const stagingRoot = await mkdtemp(path.join(canonicalPackageRoot, stagingDirectoryPrefix));
  try {
    for (const flavor of flavors) {
      const {
        label,
        generatedRuntimePackages,
        declaration,
        wasm,
        sourceDir,
        packageName,
        exactSourceSet,
        loaders,
        packFiles,
      } = flavor;
      const packageDir = path.join(canonicalPackageRoot, packageName);
      const stagedPackageDir = path.join(stagingRoot, label);
      flavor.packageDir = packageDir;
      flavor.stagedPackageDir = stagedPackageDir;
      await copyWasiPackageForStaging({
        packageDir,
        stagedPackageDir,
        wasm,
      });

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
        assert.match(managedWorkerd, /isCurrentThreadHostRegistrationActive/);
        assert.match(managedWorkerd, /reserveCurrentThreadHostRegistration/);
        assert.match(managedWorkerd, /registerCurrentThreadTaskHost/);
        assert.match(managedWorkerd, /unregisterCurrentThreadTaskHost/);
        assert.match(managedWorkerd, /__actualVersion !== 4/);
        assert.match(managedWorkerd, /Reflect\.apply\(__reserve, __binding, \[\]\)/);
        assert.match(managedWorkerd, /Reflect\.apply\(__register, __binding, __registration\)/);
        assert.match(managedWorkerd, /Reflect\.apply\(__unregister, __binding, __registration\)/);
        assert.doesNotMatch(
          managedWorkerd,
          /driveCurrentThreadRuntimeTasks|cancelCurrentThreadRuntimeTaskDispatch|dispatchHigh|dispatchLow/,
        );
        assert.match(managedWorkerd, /registerTimerHost/);
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

    for (const { packageDir, stagedPackageDir } of flavors) {
      await rm(packageDir, { force: true, recursive: true });
      await rename(stagedPackageDir, packageDir);
    }
  } finally {
    await rm(stagingRoot, { force: true, recursive: true });
  }

  console.log(
    `Staged self-contained WASI loaders in ${flavors
      .map(({ packageName }) => path.relative(repoRoot, path.join(packageRoot, packageName)))
      .join(' and ')}`,
  );
}

export async function ensureWasiPackageDirectories({
  packageNames,
  packageRoot,
  rolldownRoot,
  createNpmDirs = (npmDir) =>
    napiCli.createNpmDirs({
      cwd: rolldownRoot,
      npmDir,
      packageJsonPath: 'package.json',
    }),
}) {
  const packageStats = await Promise.all(
    packageNames.map((packageName) => lstatIfExists(path.join(packageRoot, packageName))),
  );
  for (const [index, stats] of packageStats.entries()) {
    assertOptionalDirectory(
      stats,
      path.join(packageRoot, packageNames[index]),
      'Generated WASI package',
    );
  }

  const missingPackageNames = packageNames.filter((_, index) => !packageStats[index]);
  if (missingPackageNames.length === 0) return;

  const bootstrapRoot = await mkdtemp(
    path.join(packageRoot, `${stagingDirectoryPrefix}bootstrap-`),
  );
  try {
    await createNpmDirs(bootstrapRoot);
    for (const packageName of missingPackageNames) {
      const generatedPackage = path.join(bootstrapRoot, packageName);
      await assertDirectory(generatedPackage, 'Generated WASI package');
      await rename(generatedPackage, path.join(packageRoot, packageName));
    }
  } finally {
    await rm(bootstrapRoot, { force: true, recursive: true });
  }
}

export async function copyWasiPackageForStaging({ packageDir, stagedPackageDir, wasm }) {
  await cp(packageDir, stagedPackageDir, { recursive: true });
  await assertStagedPackageTree(stagedPackageDir);

  const stagedWasm = path.join(stagedPackageDir, path.basename(wasm));
  const stagedWasmStats = await lstatIfExists(stagedWasm);
  if (stagedWasmStats && !stagedWasmStats.isFile()) {
    throw new Error(`Staged WASI binary is not a regular file: ${stagedWasm}`);
  }
  if (!stagedWasmStats) await copyFile(wasm, stagedWasm);
}

async function assertStagedPackageTree(stagedPackageDir) {
  await assertDirectory(stagedPackageDir, 'Staged WASI package');
  const pending = [stagedPackageDir];
  while (pending.length > 0) {
    const directory = pending.pop();
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const candidate = path.join(directory, entry.name);
      const stats = await lstat(candidate);
      if (stats.isSymbolicLink()) {
        throw new Error(`Staged WASI package entry must not be a symlink: ${candidate}`);
      }
      if (stats.isDirectory()) {
        pending.push(candidate);
      } else if (!stats.isFile()) {
        throw new Error(`Staged WASI package entry is not a regular file: ${candidate}`);
      }
    }
  }
  await Promise.all([
    assertRegularFile(path.join(stagedPackageDir, 'package.json'), 'Staged WASI package manifest'),
    assertRegularFile(path.join(stagedPackageDir, 'README.md'), 'Staged WASI package README'),
  ]);
}

async function removeOrphanedStagingDirectories(packageRoot) {
  for (const entry of await readdir(packageRoot, { withFileTypes: true })) {
    if (!entry.name.startsWith(stagingDirectoryPrefix)) continue;
    const candidate = path.join(packageRoot, entry.name);
    if (!entry.isDirectory()) {
      throw new Error(`WASI package staging path is not a directory: ${candidate}`);
    }
    await rm(candidate, { force: true, recursive: true });
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  await stageWasiPackages();
}
