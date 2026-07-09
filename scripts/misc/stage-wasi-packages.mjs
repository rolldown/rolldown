import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import {
  copyFile,
  cp,
  lstat,
  mkdir,
  mkdtemp,
  open,
  readFile,
  readdir,
  realpath,
  rename,
  rm,
  writeFile,
} from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';

import { parse } from 'acorn';

const defaultRepoRoot = fileURLToPath(new URL('../../', import.meta.url));
const coreRuntimePackages = ['@emnapi/core', '@emnapi/runtime', '@napi-rs/wasm-runtime'];
const runtimePackages = [...coreRuntimePackages, 'buffer'];
const notices = ['LICENSE', 'THIRD-PARTY-LICENSE'];
const transactionLockName = '.stage-wasi-packages.lock';
const transactionJournalName = '.stage-wasi-packages.transaction';
const transactionStateName = 'state.json';
const stagingDirectoryPrefix = '.stage-wasi-packages-';
const transactionStateVersion = 1;
const transactionLockTimeoutMs = 60_000;
const incompleteLockGracePeriodMs = 5_000;
const transactionLockPollMs = 20;
const maximumTransactionReplacements = 64;

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

  const transactionRoot = path.dirname(path.resolve(replacements[0].destination));
  return withStageWasiPackageLock(transactionRoot, async (canonicalRoot) => {
    const normalized = await normalizeDirectoryReplacements(canonicalRoot, replacements);
    return replaceDirectoriesTransactionallyUnlocked(canonicalRoot, normalized, {
      afterOperation,
    });
  });
}

async function replaceDirectoriesTransactionallyUnlocked(
  transactionRoot,
  replacements,
  { afterOperation } = {},
) {
  const journalRoot = transactionJournalPath(transactionRoot);
  const backupRoot = path.join(journalRoot, 'backups');
  await mkdir(journalRoot);
  await mkdir(backupRoot);

  const state = {
    version: transactionStateVersion,
    status: 'active',
    replacements: replacements.map(({ destination, staged }) => ({
      destination: managedRelativePath(transactionRoot, destination, 'Transaction destination'),
      staged: managedRelativePath(transactionRoot, staged, 'Staged package'),
    })),
  };
  await writeJsonAtomic(path.join(journalRoot, transactionStateName), state);

  try {
    for (const [index, replacement] of replacements.entries()) {
      await rename(replacement.destination, path.join(backupRoot, String(index)));
      await afterOperation?.('backup', index);
      await rename(replacement.staged, replacement.destination);
      await afterOperation?.('install', index);
    }

    await writeJsonAtomic(path.join(journalRoot, transactionStateName), {
      ...state,
      status: 'committed',
    });
  } catch (error) {
    const rollbackErrors = await rollbackDirectoryTransaction(transactionRoot, state);
    if (rollbackErrors.length > 0) {
      throw new AggregateError(
        [error, ...rollbackErrors],
        `WASI package transaction failed and rollback was incomplete; recovery state is preserved at ${journalRoot}`,
      );
    }
    try {
      await removeTransactionJournal(transactionRoot);
    } catch (cleanupError) {
      throw new AggregateError(
        [error, cleanupError],
        `WASI package transaction failed and its recovered journal could not be removed at ${journalRoot}`,
      );
    }
    throw error;
  }

  await removeTransactionJournal(transactionRoot);
}

async function withStageWasiPackageLock(transactionRoot, operation) {
  const canonicalRoot = await realpath(transactionRoot);
  const release = await acquireStageWasiPackageLock(canonicalRoot);
  let operationError;
  let result;
  try {
    await recoverInterruptedDirectoryTransaction(canonicalRoot);
    result = await operation(canonicalRoot);
  } catch (error) {
    operationError = error;
  }

  try {
    await release();
  } catch (releaseError) {
    if (operationError) {
      throw new AggregateError(
        [operationError, releaseError],
        'WASI package operation failed and its transaction lock could not be released',
      );
    }
    throw releaseError;
  }

  if (operationError) throw operationError;
  return result;
}

async function acquireStageWasiPackageLock(transactionRoot) {
  const lockPath = path.join(transactionRoot, transactionLockName);
  const ownerPath = path.join(lockPath, 'owner.json');
  const deadline = Date.now() + transactionLockTimeoutMs;

  while (true) {
    try {
      await mkdir(lockPath);
    } catch (error) {
      if (!isNodeError(error) || error.code !== 'EEXIST') throw error;
      if (await reclaimStaleStageWasiPackageLock(lockPath, ownerPath)) continue;
      if (Date.now() >= deadline) {
        throw new Error(`Timed out waiting for the WASI package transaction lock at ${lockPath}`);
      }
      await delay(transactionLockPollMs);
      continue;
    }

    const token = randomUUID();
    try {
      await writeJsonAtomic(ownerPath, {
        version: 1,
        createdAt: Date.now(),
        pid: process.pid,
        token,
      });
    } catch (error) {
      await rm(lockPath, { force: true, recursive: true });
      throw error;
    }

    return async () => {
      const owner = await readJsonIfExists(ownerPath);
      if (!owner || owner.token !== token || owner.pid !== process.pid) {
        throw new Error(`Lost ownership of the WASI package transaction lock at ${lockPath}`);
      }
      await rm(lockPath, { force: true, recursive: true });
    };
  }
}

async function reclaimStaleStageWasiPackageLock(lockPath, ownerPath) {
  const lockStats = await lstatIfExists(lockPath);
  if (!lockStats) return true;
  if (!lockStats.isDirectory()) {
    throw new Error(`WASI package transaction lock is not a directory: ${lockPath}`);
  }

  const owner = await readLockOwnerIfExists(ownerPath);
  const validOwner =
    owner &&
    owner.version === 1 &&
    Number.isSafeInteger(owner.createdAt) &&
    owner.createdAt > 0 &&
    Number.isSafeInteger(owner.pid) &&
    owner.pid > 0 &&
    typeof owner.token === 'string';
  const stale = validOwner
    ? !processExists(owner.pid)
    : Date.now() - lockStats.mtimeMs >= incompleteLockGracePeriodMs;
  if (!stale) return false;

  const stalePath = `${lockPath}.stale.${randomUUID()}`;
  try {
    await rename(lockPath, stalePath);
  } catch (error) {
    if (isNodeError(error) && error.code === 'ENOENT') return true;
    return false;
  }
  await rm(stalePath, { force: true, recursive: true });
  return true;
}

async function recoverInterruptedDirectoryTransaction(transactionRoot) {
  const journalRoot = transactionJournalPath(transactionRoot);
  const journalStats = await lstatIfExists(journalRoot);
  if (!journalStats) return;
  if (!journalStats.isDirectory()) {
    throw new Error(`WASI package transaction journal is not a directory: ${journalRoot}`);
  }

  const state = await readJsonIfExists(path.join(journalRoot, transactionStateName));
  if (!state) {
    await rm(journalRoot, { force: true, recursive: true });
    return;
  }

  const normalizedState = normalizeTransactionState(transactionRoot, state);
  if (normalizedState.status === 'committed') {
    for (const replacement of normalizedState.replacements) {
      await assertDirectory(replacement.destination, 'Committed WASI package destination');
    }
    await removeTransactionJournal(transactionRoot);
    return;
  }

  const rollbackErrors = await rollbackDirectoryTransaction(transactionRoot, state);
  if (rollbackErrors.length > 0) {
    throw new AggregateError(
      rollbackErrors,
      `Failed to recover an interrupted WASI package transaction; recovery state is preserved at ${journalRoot}`,
    );
  }
  await removeTransactionJournal(transactionRoot);
}

async function rollbackDirectoryTransaction(transactionRoot, state) {
  const normalizedState = normalizeTransactionState(transactionRoot, state);
  const backupRoot = path.join(transactionJournalPath(transactionRoot), 'backups');
  const rollbackErrors = [];

  for (let index = normalizedState.replacements.length - 1; index >= 0; index--) {
    const replacement = normalizedState.replacements[index];
    const backup = path.join(backupRoot, String(index));
    try {
      const [backupStats, destinationStats, stagedStats] = await Promise.all([
        lstatIfExists(backup),
        lstatIfExists(replacement.destination),
        lstatIfExists(replacement.staged),
      ]);
      assertOptionalDirectory(backupStats, backup, 'WASI package transaction backup');
      assertOptionalDirectory(
        destinationStats,
        replacement.destination,
        'WASI package destination',
      );
      assertOptionalDirectory(stagedStats, replacement.staged, 'Staged WASI package');

      if (backupStats) {
        if (destinationStats) {
          if (stagedStats) {
            throw new Error(
              `Cannot recover ${replacement.destination}: both destination and staged package exist`,
            );
          }
          await mkdir(path.dirname(replacement.staged), { recursive: true });
          await rename(replacement.destination, replacement.staged);
        }
        await rename(backup, replacement.destination);
      } else if (!destinationStats) {
        throw new Error(
          `Cannot recover ${replacement.destination}: destination and original backup are both missing`,
        );
      } else if (!stagedStats) {
        throw new Error(
          `Cannot recover ${replacement.destination}: staged package and original backup are both missing`,
        );
      }
    } catch (error) {
      rollbackErrors.push(
        new Error(`Failed to restore WASI package destination ${replacement.destination}`, {
          cause: error,
        }),
      );
    }
  }

  return rollbackErrors;
}

async function normalizeDirectoryReplacements(transactionRoot, replacements) {
  if (replacements.length > maximumTransactionReplacements) {
    throw new Error(
      `WASI package transaction has ${replacements.length} replacements; maximum is ${maximumTransactionReplacements}`,
    );
  }

  const destinations = new Set();
  const stagedPackages = new Set();
  const normalized = [];
  for (const replacement of replacements) {
    const unresolvedDestination = path.resolve(replacement.destination);
    const unresolvedStaged = path.resolve(replacement.staged);
    await Promise.all([
      assertDirectory(unresolvedDestination, 'WASI package transaction destination'),
      assertDirectory(unresolvedStaged, 'Staged WASI package'),
    ]);
    const [destination, staged] = await Promise.all([
      realpath(unresolvedDestination),
      realpath(unresolvedStaged),
    ]);
    const destinationRelative = managedRelativePath(
      transactionRoot,
      destination,
      'Transaction destination',
    );
    managedRelativePath(transactionRoot, staged, 'Staged package');
    if (destinationRelative.includes(path.sep)) {
      throw new Error(`WASI package destination must be a direct child of ${transactionRoot}`);
    }
    if (destinations.has(destination)) {
      throw new Error(`Duplicate WASI package transaction destination: ${destination}`);
    }
    if (stagedPackages.has(staged)) {
      throw new Error(`Duplicate staged WASI package path: ${staged}`);
    }
    destinations.add(destination);
    stagedPackages.add(staged);
    normalized.push({ destination, staged });
  }
  assertReplacementPathsDoNotOverlap(destinations, stagedPackages);
  return normalized;
}

function normalizeTransactionState(transactionRoot, state) {
  if (
    state?.version !== transactionStateVersion ||
    (state.status !== 'active' && state.status !== 'committed') ||
    !Array.isArray(state.replacements) ||
    state.replacements.length === 0 ||
    state.replacements.length > maximumTransactionReplacements
  ) {
    throw new Error(
      `Invalid WASI package transaction state in ${transactionJournalPath(transactionRoot)}`,
    );
  }

  const destinations = new Set();
  const stagedPackages = new Set();
  const replacements = state.replacements.map((replacement) => {
    const destination = resolveManagedRelativePath(
      transactionRoot,
      replacement?.destination,
      'Transaction destination',
    );
    const staged = resolveManagedRelativePath(
      transactionRoot,
      replacement?.staged,
      'Staged package',
    );
    const destinationRelative = path.relative(transactionRoot, destination);
    if (destinationRelative.includes(path.sep)) {
      throw new Error(
        `Recovered WASI package destination must be a direct child of ${transactionRoot}`,
      );
    }
    if (destinations.has(destination) || stagedPackages.has(staged)) {
      throw new Error('Recovered WASI package transaction contains duplicate paths');
    }
    destinations.add(destination);
    stagedPackages.add(staged);
    return { destination, staged };
  });
  assertReplacementPathsDoNotOverlap(destinations, stagedPackages);

  return { ...state, replacements };
}

function assertReplacementPathsDoNotOverlap(destinations, stagedPackages) {
  for (const destination of destinations) {
    for (const staged of stagedPackages) {
      if (pathsOverlap(destination, staged)) {
        throw new Error(
          `WASI package destination and staged paths overlap: ${destination}, ${staged}`,
        );
      }
    }
  }
}

async function removeTransactionJournal(transactionRoot) {
  const journalRoot = transactionJournalPath(transactionRoot);
  await rm(path.join(journalRoot, 'backups'), { force: true, recursive: true });
  await rm(path.join(journalRoot, transactionStateName), { force: true });
  await rm(journalRoot, { force: true, recursive: true });
}

function transactionJournalPath(transactionRoot) {
  return path.join(transactionRoot, transactionJournalName);
}

function managedRelativePath(transactionRoot, candidate, label) {
  const relative = path.relative(transactionRoot, candidate);
  if (
    relative === '' ||
    relative === '..' ||
    relative.startsWith(`..${path.sep}`) ||
    path.isAbsolute(relative)
  ) {
    throw new Error(`${label} escapes ${transactionRoot}: ${candidate}`);
  }
  const firstSegment = relative.split(path.sep, 1)[0];
  if (firstSegment === transactionLockName || firstSegment === transactionJournalName) {
    throw new Error(`${label} overlaps WASI package transaction state: ${candidate}`);
  }
  return relative;
}

function resolveManagedRelativePath(transactionRoot, relative, label) {
  if (typeof relative !== 'string' || relative.length === 0 || path.isAbsolute(relative)) {
    throw new Error(`Invalid ${label.toLowerCase()} in WASI package transaction journal`);
  }
  const resolved = path.resolve(transactionRoot, relative);
  managedRelativePath(transactionRoot, resolved, label);
  return resolved;
}

function isSameOrDescendant(parent, candidate) {
  const relative = path.relative(parent, candidate);
  return relative === '' || (!relative.startsWith(`..${path.sep}`) && relative !== '..');
}

function pathsOverlap(first, second) {
  return isSameOrDescendant(first, second) || isSameOrDescendant(second, first);
}

async function assertDirectory(candidate, label) {
  const stats = await lstat(candidate);
  if (!stats.isDirectory()) throw new Error(`${label} is not a directory: ${candidate}`);
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

async function readJsonIfExists(candidate) {
  try {
    return JSON.parse(await readFile(candidate, 'utf8'));
  } catch (error) {
    if (isNodeError(error) && error.code === 'ENOENT') return undefined;
    throw error;
  }
}

async function readLockOwnerIfExists(candidate) {
  try {
    return await readJsonIfExists(candidate);
  } catch (error) {
    if (error instanceof SyntaxError) return undefined;
    throw error;
  }
}

async function writeJsonAtomic(destination, value) {
  const temporary = path.join(
    path.dirname(destination),
    `.${path.basename(destination)}.${process.pid}.${randomUUID()}.tmp`,
  );
  let created = false;
  let renamed = false;
  try {
    const handle = await open(temporary, 'wx', 0o600);
    created = true;
    try {
      await handle.writeFile(`${JSON.stringify(value)}\n`, 'utf8');
      await handle.sync();
    } finally {
      await handle.close();
    }
    await rename(temporary, destination);
    renamed = true;
  } finally {
    if (created && !renamed) await rm(temporary, { force: true });
  }
}

function processExists(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return isNodeError(error) && error.code === 'EPERM';
  }
}

function isNodeError(error) {
  return error instanceof Error && 'code' in error;
}

export async function stageWasiPackages({ repoRoot = defaultRepoRoot, transactionHook } = {}) {
  const packageRoot = path.join(repoRoot, 'packages/rolldown/npm');
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

  await withStageWasiPackageLock(packageRoot, async (canonicalPackageRoot) => {
    await removeOrphanedStagingDirectories(canonicalPackageRoot);
    const stagingRoot = await mkdtemp(path.join(canonicalPackageRoot, stagingDirectoryPrefix));
    try {
      for (const flavor of flavors) {
        const {
          label,
          generatedRuntimePackages,
          declaration,
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

      const replacements = await normalizeDirectoryReplacements(
        canonicalPackageRoot,
        flavors.map(({ packageDir, stagedPackageDir }) => ({
          destination: packageDir,
          staged: stagedPackageDir,
        })),
      );
      await replaceDirectoriesTransactionallyUnlocked(canonicalPackageRoot, replacements, {
        afterOperation: transactionHook,
      });
    } finally {
      if (!(await lstatIfExists(transactionJournalPath(canonicalPackageRoot)))) {
        await rm(stagingRoot, { force: true, recursive: true });
      }
    }
  });

  console.log(
    `Staged self-contained WASI loaders in ${flavors
      .map(({ packageName }) => path.relative(repoRoot, path.join(packageRoot, packageName)))
      .join(' and ')}`,
  );
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
