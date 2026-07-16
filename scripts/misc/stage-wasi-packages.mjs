import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { constants as fsConstants } from 'node:fs';
import {
  chmod,
  copyFile,
  cp,
  lstat,
  mkdir,
  mkdtemp,
  open,
  readFile,
  readdir,
  readlink,
  realpath,
  rename,
  rm,
  writeFile,
} from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';

import { NapiCli } from '@napi-rs/cli';
import { parse } from 'acorn';

const defaultRepoRoot = fileURLToPath(new URL('../../', import.meta.url));
const coreRuntimePackages = ['@emnapi/core', '@emnapi/runtime', '@napi-rs/wasm-runtime'];
const runtimePackages = [...coreRuntimePackages, 'buffer'];
const notices = ['LICENSE', 'THIRD-PARTY-LICENSE'];
const transactionLockName = '.stage-wasi-packages.lock';
const transactionLockCandidatePrefix = `${transactionLockName}.candidate.`;
const transactionLockPreparationPrefix = `${transactionLockName}.candidate-preparing.`;
const transactionLockPreparationVersion = 2;
const transactionLockRetiredPrefix = `${transactionLockName}.retired.`;
const transactionReclaimCandidatePrefix = `${transactionLockName}.reclaim.`;
const transactionReclaimPreparationPrefix = `${transactionLockName}.reclaim-preparing.`;
const transactionReclaimPreparationVersion = 2;
const transactionReclaimTicketName = 'ticket.json';
const transactionJournalName = '.stage-wasi-packages.transaction';
const transactionStateName = 'state.json';
const stagingDirectoryPrefix = '.stage-wasi-packages-';
const transactionStateVersion = 1;
const transactionDirectoryMode = 0o775;
const transactionMetadataMode = 0o644;
const transactionLockOwnerMaximumBytes = 16 * 1024;
const transactionReclaimTicketMaximumBytes = 4 * 1024;
const transactionStateMaximumBytes = 1024 * 1024;
const transactionLockTimeoutMs = 60_000;
const transactionLockPollMs = 20;
const transactionLockCleanupMaxRetries = 5;
const transactionLockCleanupTimeoutMs = 5_000;
const processIncarnationProbeTimeoutMs = 5_000;
const processIncarnationMatchCacheMs = 1_000;
const maximumTransactionReplacements = 64;
const execFileAsync = promisify(execFile);
const napiCli = new NapiCli();
const processIncarnationMatchCache = new Map();
const unavailableExecutionIdentity = 'unavailable';
let currentExecutionIdentityPromise;
let currentProcessIncarnationPromise;

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
  // emnapi v2 shapes (patches/@emnapi__core@2.0.0-alpha.2.patch): the
  // reentrant-call result writes re-create their DataView after calling into
  // JavaScript (napi_call_function + napi_new_instance).
  const callbackResultWrites =
    code.match(
      /v = emnapiCtx\.napiValueFromJsValue\(ret\);[^]*?new DataView\(wasmMemory\.buffer\)\.setUint32\(result, v, true\)/g,
    ) ?? [];
  assert.ok(
    callbackResultWrites.length >= 2,
    `${loader} does not contain hardened napi_call_function/napi_new_instance result writes`,
  );
  // TSFN plugin enqueue re-creates its Int32Array view at every use (the v2
  // plugin replaces the v1 `state()`/`ensureBufferFor` helper with `i32a()`).
  assert.match(code, /const i32a = \(\) => new Int32Array\([\w$.]*wasmMemory\.buffer\)/);
  assert.match(code, /Atomics\.exchange\(i32a\(\), scheduled >>> 2, 1\)/);
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
  await createCooperativeDirectory(journalRoot);
  await syncDirectory(transactionRoot);
  await afterOperation?.('journal-root', -1);
  await createCooperativeDirectory(backupRoot);
  await syncDirectory(journalRoot);
  await afterOperation?.('backup-root', -1);

  const state = {
    version: transactionStateVersion,
    status: 'active',
    replacements: replacements.map(({ destination, staged }) => ({
      destination: managedRelativePath(transactionRoot, destination, 'Transaction destination'),
      staged: managedRelativePath(transactionRoot, staged, 'Staged package'),
    })),
  };
  await writeJsonAtomic(path.join(journalRoot, transactionStateName), state);
  await afterOperation?.('journal', -1);

  try {
    for (const [index, replacement] of replacements.entries()) {
      await renameDurably(replacement.destination, path.join(backupRoot, String(index)));
      await afterOperation?.('backup', index);
      await renameDurably(replacement.staged, replacement.destination);
      await afterOperation?.('install', index);
    }

    await writeJsonAtomic(path.join(journalRoot, transactionStateName), {
      ...state,
      status: 'committed',
    });
    await afterOperation?.('commit', -1);
  } catch (error) {
    const rollbackErrors = await rollbackDirectoryTransaction(transactionRoot, state);
    if (rollbackErrors.length > 0) {
      throw aggregateErrors(
        [error, ...rollbackErrors],
        `WASI package transaction failed and rollback was incomplete; recovery state is preserved at ${journalRoot}`,
      );
    }
    try {
      await removeTransactionJournal(transactionRoot, { afterOperation });
    } catch (cleanupError) {
      throw aggregateErrors(
        [error, cleanupError],
        `WASI package transaction failed and its recovered journal could not be removed at ${journalRoot}`,
      );
    }
    throw error;
  }

  await removeTransactionJournal(transactionRoot, { afterOperation });
}

export async function withStageWasiPackageLock(
  transactionRoot,
  operation,
  {
    afterLockPublishFailure,
    afterLockCandidateCreate,
    afterLockCandidatePreparationCreate,
    afterLockCandidateRetire,
    afterLockRetire,
    afterReclaimGuardCandidateCreate,
    afterReclaimGuardPreparationCreate,
    afterReclaimGuardRetire,
    afterReclaimGuardTicketPublish,
    afterStaleLockRetire,
    afterStaleLockObserved,
    beforeLockPublish,
    beforeReclaimGuardRetire,
  } = {},
) {
  const createdTransactionRoot = await mkdir(transactionRoot, {
    mode: transactionDirectoryMode,
    recursive: true,
  });
  if (createdTransactionRoot && process.platform !== 'win32') {
    await chmod(transactionRoot, transactionDirectoryMode);
  }
  await assertDirectory(transactionRoot, 'WASI package transaction root');
  const canonicalRoot = await realpath(transactionRoot);
  const release = await acquireStageWasiPackageLock(canonicalRoot, {
    afterLockPublishFailure,
    afterLockCandidateCreate,
    afterLockCandidatePreparationCreate,
    afterLockCandidateRetire,
    afterLockRetire,
    afterReclaimGuardCandidateCreate,
    afterReclaimGuardPreparationCreate,
    afterReclaimGuardRetire,
    afterReclaimGuardTicketPublish,
    afterStaleLockRetire,
    afterStaleLockObserved,
    beforeLockPublish,
    beforeReclaimGuardRetire,
  });
  let operationFailed = false;
  let operationError;
  let result;
  try {
    await removeRetiredStageWasiPackageLocks(canonicalRoot);
    await reclaimStaleStageWasiPackageLockPreparations(canonicalRoot);
    await reclaimStaleStageWasiPackageLockCandidates(canonicalRoot);
    await reclaimStaleStageWasiPackageReclaimPreparations(canonicalRoot, {
      afterReclaimGuardRetire,
      beforeReclaimGuardRetire,
    });
    await reclaimDeadStageWasiPackageReclaimCandidates(canonicalRoot, {
      afterReclaimGuardRetire,
      beforeReclaimGuardRetire,
    });
    await recoverInterruptedDirectoryTransaction(canonicalRoot);
    result = await operation(canonicalRoot);
  } catch (error) {
    operationFailed = true;
    operationError = error;
  }

  try {
    await release();
  } catch (releaseError) {
    if (operationFailed) {
      throw aggregateErrors(
        [operationError, releaseError],
        'WASI package operation failed and its transaction lock could not be released',
      );
    }
    throw releaseError;
  }

  if (operationFailed) throw operationError;
  return result;
}

async function acquireStageWasiPackageLock(
  transactionRoot,
  {
    afterLockPublishFailure,
    afterLockCandidateCreate,
    afterLockCandidatePreparationCreate,
    afterLockCandidateRetire,
    afterLockRetire,
    afterReclaimGuardCandidateCreate,
    afterReclaimGuardPreparationCreate,
    afterReclaimGuardRetire,
    afterReclaimGuardTicketPublish,
    afterStaleLockRetire,
    afterStaleLockObserved,
    beforeLockPublish,
    beforeReclaimGuardRetire,
  } = {},
) {
  const lockPath = path.join(transactionRoot, transactionLockName);
  const ownerPath = path.join(lockPath, 'owner.json');
  const executionIdentity = await getCurrentExecutionIdentity();
  const deadline = Date.now() + transactionLockTimeoutMs;

  while (true) {
    const token = randomUUID();
    const expectedOwner = createStageWasiPackageLockOwner(token, executionIdentity);
    const candidateLockPath = path.join(
      transactionRoot,
      `${transactionLockCandidatePrefix}${process.pid}.${token}`,
    );
    const preparationLockPath = path.join(
      transactionRoot,
      stageWasiPackageLockPreparationName(process.pid, executionIdentity, token),
    );
    const preparationOwnerPath = path.join(preparationLockPath, 'owner.json');
    let candidatePublished = false;
    let canonicalPublished = false;
    let publishingLockCandidate = false;
    try {
      // See internal-docs/async-runtime/implementation.md.
      await createCooperativeDirectory(preparationLockPath);
      await afterLockCandidatePreparationCreate?.(preparationLockPath);
      await writeJsonAtomic(preparationOwnerPath, expectedOwner);
      await rename(preparationLockPath, candidateLockPath);
      candidatePublished = true;
      await syncDirectory(transactionRoot);
      await afterLockCandidateCreate?.(candidateLockPath);
      await beforeLockPublish?.({ candidateLockPath, lockPath });
      if (process.platform !== 'win32' && (await lstatIfExists(lockPath))) {
        const error = new Error(`WASI package transaction lock already exists: ${lockPath}`);
        error.code = 'EEXIST';
        throw error;
      }
      publishingLockCandidate = true;
      await rename(candidateLockPath, lockPath);
      canonicalPublished = true;
      await syncDirectory(transactionRoot);
    } catch (error) {
      try {
        if (canonicalPublished) {
          await retireOwnedStageWasiPackageCanonicalLock(
            lockPath,
            ownerPath,
            expectedOwner,
            transactionRoot,
            {
              afterRetire: afterLockCandidateRetire,
              label: 'WASI package transaction lock publication',
            },
          );
        } else {
          await retireStageWasiPackageLockCandidate(
            candidatePublished ? candidateLockPath : preparationLockPath,
            transactionRoot,
            {
              afterLockCandidateRetire,
            },
          );
        }
      } catch (cleanupError) {
        throw aggregateErrors(
          [error, cleanupError],
          'WASI package transaction lock publication failed and its candidate could not be cleaned up',
        );
      }
      if (publishingLockCandidate) {
        await afterLockPublishFailure?.({ error, lockPath });
      }
      if (isNodeError(error) && error.code === 'ENOENT') {
        await assertDirectory(transactionRoot, 'WASI package transaction root');
        if (Date.now() >= deadline) {
          throw new Error(
            `Timed out publishing a WASI package transaction lock candidate in ${transactionRoot}`,
          );
        }
        await delay(transactionLockPollMs);
        continue;
      }
      if (!isLockAlreadyExistsError(error)) throw error;
      if (error.code === 'EPERM' && !publishingLockCandidate) throw error;
      if (publishingLockCandidate && error.code === 'EPERM' && !(await lstatIfExists(lockPath))) {
        if (Date.now() >= deadline) throw error;
        await delay(transactionLockPollMs);
        continue;
      }
      if (
        await reclaimStaleStageWasiPackageLock(lockPath, ownerPath, transactionRoot, {
          afterReclaimGuardCandidateCreate,
          afterReclaimGuardPreparationCreate,
          afterReclaimGuardRetire,
          afterReclaimGuardTicketPublish,
          afterStaleLockRetire,
          afterStaleLockObserved,
          beforeReclaimGuardRetire,
        })
      ) {
        continue;
      }
      if (Date.now() >= deadline) {
        throw new Error(`Timed out waiting for the WASI package transaction lock at ${lockPath}`);
      }
      await delay(transactionLockPollMs);
      continue;
    }

    return () =>
      retireOwnedStageWasiPackageCanonicalLock(
        lockPath,
        ownerPath,
        expectedOwner,
        transactionRoot,
        {
          afterRetire: afterLockRetire,
          label: 'WASI package transaction lock',
        },
      );
  }
}

async function retireOwnedStageWasiPackageCanonicalLock(
  lockPath,
  ownerPath,
  expectedOwner,
  transactionRoot,
  { afterRetire, label },
) {
  const retiredPath = stageWasiPackageLockRetiredPath(transactionRoot);
  const retired = await renameStageWasiPackagePathWithRetry(lockPath, retiredPath, {
    async beforeAttempt() {
      const owner = await readStageWasiPackageLockOwner(ownerPath, {
        required: true,
      });
      if (!sameStageWasiPackageLockOwner(owner, expectedOwner)) {
        throw new Error(`Lost ownership of the ${label} at ${lockPath}`);
      }
    },
  });
  if (!retired) {
    throw new Error(`Lost ownership of the ${label} at ${lockPath}`);
  }
  let retireHookFailed = false;
  let retireHookError;
  try {
    await afterRetire?.(retiredPath);
  } catch (error) {
    retireHookFailed = true;
    retireHookError = error;
  }
  try {
    await removeRetiredStageWasiPackageLock(retiredPath);
  } catch (cleanupError) {
    if (retireHookFailed) {
      throw aggregateErrors(
        [retireHookError, cleanupError],
        `${label} retirement failed at ${retiredPath}`,
      );
    }
    throw cleanupError;
  }
  if (retireHookFailed) throw retireHookError;
}

function isLockAlreadyExistsError(error) {
  return (
    isNodeError(error) &&
    (error.code === 'EEXIST' || error.code === 'ENOTEMPTY' || error.code === 'EPERM')
  );
}

async function reclaimStaleStageWasiPackageLock(
  lockPath,
  ownerPath,
  transactionRoot,
  {
    afterReclaimGuardCandidateCreate,
    afterReclaimGuardPreparationCreate,
    afterReclaimGuardRetire,
    afterReclaimGuardTicketPublish,
    afterStaleLockRetire,
    afterStaleLockObserved,
    beforeReclaimGuardRetire,
  } = {},
) {
  const lockStats = await lstatIfExists(lockPath);
  if (!lockStats) return true;
  if (!lockStats.isDirectory()) {
    throw new Error(`WASI package transaction lock is not a directory: ${lockPath}`);
  }

  const observedOwner = await readStageWasiPackageLockOwner(ownerPath, { required: true });
  assertSupportedCanonicalStageWasiPackageLockOwner(observedOwner, ownerPath);
  if (!(await stageWasiPackageLockIsStale(observedOwner))) return false;
  await afterStaleLockObserved?.({ owner: observedOwner });

  const releaseReclaimGuard = await acquireStageWasiPackageReclaimGuard(transactionRoot, {
    afterReclaimGuardCandidateCreate,
    afterReclaimGuardPreparationCreate,
    afterReclaimGuardRetire,
    afterReclaimGuardTicketPublish,
    beforeReclaimGuardRetire,
  });
  let retiredPath;
  try {
    const currentStats = await lstatIfExists(lockPath);
    if (!currentStats) return true;
    if (!currentStats.isDirectory()) {
      throw new Error(`WASI package transaction lock is not a directory: ${lockPath}`);
    }
    const currentOwner = await readStageWasiPackageLockOwner(ownerPath, { required: true });
    if (!sameStageWasiPackageLockOwner(currentOwner, observedOwner)) return false;
    if (!(await stageWasiPackageLockIsStale(currentOwner))) return false;

    retiredPath = stageWasiPackageLockRetiredPath(path.dirname(lockPath));
    let retired = false;
    try {
      await rename(lockPath, retiredPath);
      retired = true;
      await syncDirectory(transactionRoot);
    } catch (error) {
      if (retired) throw error;
      if (isNodeError(error) && error.code === 'ENOENT') return true;
      return false;
    }
    await afterStaleLockRetire?.(retiredPath);
  } finally {
    await releaseReclaimGuard();
  }
  if (retiredPath) await removeRetiredStageWasiPackageLock(retiredPath);
  return true;
}

async function reclaimStaleStageWasiPackageLockPreparations(transactionRoot) {
  for (const entry of await readdir(transactionRoot)) {
    if (!entry.startsWith(transactionLockPreparationPrefix)) continue;
    const identity = parseStageWasiPackageLockPreparation(entry);
    if (!identity) {
      throw new Error(`Invalid WASI package transaction lock preparation name: ${entry}`);
    }
    const preparationPath = path.join(transactionRoot, entry);
    const stats = await lstatIfExists(preparationPath);
    if (!stats) continue;
    if (!stats.isDirectory()) {
      throw new Error(
        `WASI package transaction lock preparation is not a directory: ${preparationPath}`,
      );
    }

    const owner = await readStageWasiPackageLockOwner(path.join(preparationPath, 'owner.json'));
    const ownerIsValid =
      owner?.version === 2 &&
      owner.pid === identity.pid &&
      owner.token === identity.token &&
      sameExecutionIdentityFingerprint(owner, identity.executionFingerprint);
    if (ownerIsValid) {
      if (await stageWasiPackageLockOwnerIsLive(owner)) continue;
    } else if (await stageWasiPackagePreparationIsLive(identity)) {
      continue;
    }

    await retireStageWasiPackageLockCandidate(preparationPath, transactionRoot);
  }
}

async function reclaimStaleStageWasiPackageLockCandidates(transactionRoot) {
  for (const entry of await readdir(transactionRoot)) {
    if (!entry.startsWith(transactionLockCandidatePrefix)) continue;
    const candidateLockPath = path.join(transactionRoot, entry);
    const candidateStats = await lstatIfExists(candidateLockPath);
    if (!candidateStats) continue;
    if (!candidateStats.isDirectory()) {
      throw new Error(
        `WASI package transaction lock candidate is not a directory: ${candidateLockPath}`,
      );
    }

    const owner = await readStageWasiPackageLockOwner(path.join(candidateLockPath, 'owner.json'));
    if (!owner) continue;
    if (!(await stageWasiPackageLockIsStale(owner))) continue;

    await retireStageWasiPackageLockCandidate(candidateLockPath, transactionRoot);
  }
}

async function removeRetiredStageWasiPackageLocks(transactionRoot) {
  for (const entry of await readdir(transactionRoot)) {
    if (!entry.startsWith(transactionLockRetiredPrefix)) continue;
    const retiredPath = path.join(transactionRoot, entry);
    const retiredStats = await lstatIfExists(retiredPath);
    if (!retiredStats) continue;
    if (!retiredStats.isDirectory()) {
      throw new Error(`Retired WASI package transaction lock is not a directory: ${retiredPath}`);
    }
    await removeRetiredStageWasiPackageLock(retiredPath);
  }
}

async function removeRetiredStageWasiPackageLock(retiredPath) {
  // Retired names are unique and never become canonical again, so bounded
  // retries cannot remove a successor-owned lock. Use an explicit loop because
  // overlapping recursive removals can surface a transient Windows EPERM after
  // Node's built-in rm retries have already stopped.
  const deadline = Date.now() + transactionLockCleanupTimeoutMs;
  let retry = 0;
  while (true) {
    try {
      await rm(retiredPath, { force: true, recursive: true });
      return;
    } catch (error) {
      if (
        process.platform !== 'win32' ||
        !isNodeError(error) ||
        !['EACCES', 'EBUSY', 'ENOTEMPTY', 'EPERM'].includes(error.code)
      ) {
        throw error;
      }
      if (Date.now() >= deadline) throw error;
      retry++;
      await delay(Math.min(transactionLockPollMs * retry, 200));
    }
  }
}

async function retireStageWasiPackageReclaimPath(
  reclaimPath,
  transactionRoot,
  { afterReclaimGuardRetire, beforeReclaimGuardRetire } = {},
) {
  return retireStageWasiPackageOwnedPath(reclaimPath, transactionRoot, {
    afterRetire: afterReclaimGuardRetire,
    beforeRetire: beforeReclaimGuardRetire,
    label: 'WASI package reclaim guard',
  });
}

async function retireStageWasiPackageLockCandidate(
  candidateLockPath,
  transactionRoot,
  { afterLockCandidateRetire } = {},
) {
  return retireStageWasiPackageOwnedPath(candidateLockPath, transactionRoot, {
    afterRetire: afterLockCandidateRetire,
    label: 'WASI package transaction lock candidate',
  });
}

async function retireStageWasiPackageOwnedPath(
  ownedPath,
  transactionRoot,
  { afterRetire, beforeRetire, label },
) {
  const retirementErrors = [];
  try {
    await beforeRetire?.(ownedPath);
  } catch (error) {
    retirementErrors.push(error);
  }

  const retiredPath = stageWasiPackageLockRetiredPath(transactionRoot);
  let retired;
  try {
    retired = await renameStageWasiPackagePathWithRetry(ownedPath, retiredPath);
  } catch (error) {
    retirementErrors.push(error);
    throwStageWasiPackageRetirementErrors(retirementErrors, label, ownedPath);
  }
  if (!retired) {
    throwStageWasiPackageRetirementErrors(retirementErrors, label, ownedPath);
    return false;
  }

  try {
    await afterRetire?.(retiredPath);
  } catch (error) {
    retirementErrors.push(error);
  }
  try {
    await removeRetiredStageWasiPackageLock(retiredPath);
  } catch (error) {
    retirementErrors.push(error);
  }
  throwStageWasiPackageRetirementErrors(retirementErrors, label, retiredPath);
  return true;
}

async function renameStageWasiPackagePathWithRetry(
  sourcePath,
  retiredPath,
  { beforeAttempt } = {},
) {
  // UUID callers never reuse sourcePath. Canonical callers revalidate their
  // exact owner before every attempt.
  for (let retry = 0; ; retry++) {
    let renamed = false;
    try {
      await beforeAttempt?.();
      await rename(sourcePath, retiredPath);
      renamed = true;
      await syncDirectory(path.dirname(sourcePath));
      return true;
    } catch (error) {
      if (renamed) throw error;
      if (isNodeError(error) && error.code === 'ENOENT') return false;
      if (
        process.platform !== 'win32' ||
        !isNodeError(error) ||
        !['EACCES', 'EBUSY', 'EPERM'].includes(error.code) ||
        retry >= transactionLockCleanupMaxRetries
      ) {
        throw error;
      }
      await delay(transactionLockPollMs * (retry + 1));
    }
  }
}

function throwStageWasiPackageRetirementErrors(errors, label, candidate) {
  if (errors.length === 0) return;
  if (errors.length === 1) throw errors[0];
  throw aggregateErrors(errors, `${label} retirement failed at ${candidate}`);
}

function stageWasiPackageLockRetiredPath(transactionRoot) {
  return path.join(
    transactionRoot,
    `${transactionLockRetiredPrefix}${process.pid}.${randomUUID()}`,
  );
}

function createStageWasiPackageLockOwner(token, executionIdentity) {
  return {
    version: 2,
    bootId: executionIdentity.bootId,
    createdAt: Date.now(),
    incarnation: executionIdentity.incarnation,
    machineId: executionIdentity.machineId,
    pid: process.pid,
    pidNamespaceId: executionIdentity.pidNamespaceId,
    token,
  };
}

function stageWasiPackageLockOwnerIsValid(owner) {
  if (
    !owner ||
    !Number.isSafeInteger(owner.createdAt) ||
    owner.createdAt <= 0 ||
    !Number.isSafeInteger(owner.pid) ||
    owner.pid <= 0 ||
    typeof owner.token !== 'string' ||
    owner.token.length === 0 ||
    owner.token.length > 512
  ) {
    return false;
  }
  if (owner.version === 1) {
    return (
      owner.incarnation === undefined ||
      isExecutionIdentityComponent(owner.incarnation, { allowUnavailable: false })
    );
  }
  return Boolean(
    owner.version === 2 &&
    isExecutionIdentityComponent(owner.bootId) &&
    isExecutionIdentityComponent(owner.incarnation) &&
    isExecutionIdentityComponent(owner.machineId) &&
    isExecutionIdentityComponent(owner.pidNamespaceId),
  );
}

function assertSupportedCanonicalStageWasiPackageLockOwner(owner, ownerPath) {
  if (owner.version === 1) {
    throw new Error(
      `Unsupported legacy WASI package transaction lock owner at ${ownerPath}; explicit cleanup is required`,
    );
  }
}

async function stageWasiPackageLockIsStale(owner) {
  return !(await stageWasiPackageLockOwnerIsLive(owner));
}

function sameStageWasiPackageLockOwner(first, second) {
  if (!first || !second) return first === second;
  return (
    first.version === second.version &&
    first.bootId === second.bootId &&
    first.createdAt === second.createdAt &&
    first.machineId === second.machineId &&
    first.pid === second.pid &&
    first.pidNamespaceId === second.pidNamespaceId &&
    first.token === second.token &&
    first.incarnation === second.incarnation
  );
}

function isExecutionIdentityComponent(value, { allowUnavailable = true } = {}) {
  return (
    typeof value === 'string' &&
    value.length > 0 &&
    value.length <= 512 &&
    !/[\r\n]/.test(value) &&
    !value.includes('\0') &&
    (allowUnavailable || value !== unavailableExecutionIdentity)
  );
}

// Stale-lock reclaimers use unique Lamport bakery candidates, so crash cleanup
// never mutates a path that a successor can own. See
// internal-docs/async-runtime/implementation.md.
export async function acquireStageWasiPackageReclaimGuard(
  transactionRoot,
  {
    afterReclaimGuardCandidateCreate,
    afterReclaimGuardPreparationCreate,
    afterReclaimGuardRetire,
    afterReclaimGuardTicketPublish,
    beforeReclaimGuardAdmission,
    beforeReclaimGuardTicketPublish,
    beforeReclaimGuardRetire,
    probeCurrentProcessIncarnation = getCurrentProcessIncarnation,
  } = {},
) {
  const token = randomUUID();
  const currentExecutionIdentity = await getCurrentExecutionIdentity();
  const probedProcessIncarnation = await probeCurrentProcessIncarnation();
  const executionIdentity = {
    ...currentExecutionIdentity,
    incarnation:
      processIncarnationFormat(probedProcessIncarnation) === undefined
        ? unavailableExecutionIdentity
        : probedProcessIncarnation,
  };
  const candidateName = `${transactionReclaimCandidatePrefix}${process.pid}.${token}`;
  const preparationName = stageWasiPackageReclaimPreparationName(
    process.pid,
    executionIdentity,
    token,
  );
  const candidatePath = path.join(transactionRoot, candidateName);
  const preparationPath = path.join(transactionRoot, preparationName);
  const ownerPath = path.join(candidatePath, 'owner.json');
  const preparationOwnerPath = path.join(preparationPath, 'owner.json');
  const ticketPath = path.join(candidatePath, transactionReclaimTicketName);
  const owner = createStageWasiPackageLockOwner(token, executionIdentity);
  const deadline = Date.now() + transactionLockTimeoutMs;
  let candidatePublished = false;
  let ticket;

  try {
    await reclaimStaleStageWasiPackageReclaimPreparations(transactionRoot, {
      afterReclaimGuardRetire,
      beforeReclaimGuardRetire,
    });
    // Keep incomplete owner publication outside the bakery namespace, then
    // publish the complete chooser with one atomic rename.
    await createCooperativeDirectory(preparationPath);
    await afterReclaimGuardPreparationCreate?.(preparationPath);
    await writeJsonAtomic(preparationOwnerPath, owner);
    await rename(preparationPath, candidatePath);
    candidatePublished = true;
    await syncDirectory(transactionRoot);
    await afterReclaimGuardCandidateCreate?.(candidatePath);

    const choosingCandidates = await readStageWasiPackageReclaimCandidates(transactionRoot, {
      afterReclaimGuardRetire,
      beforeReclaimGuardRetire,
    });
    const maximumTicket = choosingCandidates.reduce(
      (maximum, candidate) => Math.max(maximum, candidate.ticket ?? 0),
      0,
    );
    if (maximumTicket >= Number.MAX_SAFE_INTEGER) {
      throw new Error('WASI package reclaim-guard ticket space is exhausted');
    }
    ticket = maximumTicket + 1;
    await beforeReclaimGuardTicketPublish?.({ candidatePath, ticket });
    await writeJsonAtomic(ticketPath, { ticket, version: 1 });
    await afterReclaimGuardTicketPublish?.(candidatePath);

    while (true) {
      const candidates = await readStageWasiPackageReclaimCandidates(transactionRoot, {
        afterReclaimGuardRetire,
        beforeReclaimGuardRetire,
      });
      const ownCandidate = candidates.find((candidate) => candidate.name === candidateName);
      if (
        !ownCandidate ||
        !sameStageWasiPackageLockOwner(ownCandidate.owner, owner) ||
        ownCandidate.ticket !== ticket
      ) {
        throw new Error(`Lost ownership of the WASI package reclaim guard at ${candidatePath}`);
      }

      const blocked = candidates.some(
        (candidate) =>
          candidate.name !== candidateName &&
          (candidate.ticket === undefined ||
            candidate.ticket < ticket ||
            (candidate.ticket === ticket && candidate.name < candidateName)),
      );
      if (!blocked) {
        await beforeReclaimGuardAdmission?.({ candidatePath, ticket });
        break;
      }
      if (Date.now() >= deadline) {
        throw new Error(`Timed out waiting for the WASI package reclaim guard at ${candidatePath}`);
      }
      await delay(transactionLockPollMs);
    }
  } catch (error) {
    try {
      await retireStageWasiPackageReclaimPath(
        candidatePublished ? candidatePath : preparationPath,
        transactionRoot,
        { afterReclaimGuardRetire, beforeReclaimGuardRetire },
      );
    } catch (cleanupError) {
      throw aggregateErrors(
        [error, cleanupError],
        `WASI package reclaim-guard acquisition failed and its candidate could not be cleaned up`,
      );
    }
    throw error;
  }

  return async () => {
    const [currentOwner, currentTicket] = await Promise.all([
      readStageWasiPackageLockOwner(ownerPath, { required: true }),
      readStageWasiPackageReclaimTicket(ticketPath),
    ]);
    if (!sameStageWasiPackageLockOwner(currentOwner, owner) || currentTicket !== ticket) {
      throw new Error(`Lost ownership of the WASI package reclaim guard at ${candidatePath}`);
    }
    if (
      !(await retireStageWasiPackageReclaimPath(candidatePath, transactionRoot, {
        afterReclaimGuardRetire,
        beforeReclaimGuardRetire,
      }))
    ) {
      throw new Error(`Lost ownership of the WASI package reclaim guard at ${candidatePath}`);
    }
  };
}

async function reclaimStaleStageWasiPackageReclaimPreparations(
  transactionRoot,
  { afterReclaimGuardRetire, beforeReclaimGuardRetire } = {},
) {
  for (const entry of await readdir(transactionRoot)) {
    if (!entry.startsWith(transactionReclaimPreparationPrefix)) continue;
    const identity = parseStageWasiPackageReclaimPreparation(entry);
    if (!identity) {
      throw new Error(`Invalid WASI package reclaim-guard preparation name: ${entry}`);
    }
    const preparationPath = path.join(transactionRoot, entry);
    const stats = await lstatIfExists(preparationPath);
    if (!stats) continue;
    if (!stats.isDirectory()) {
      throw new Error(
        `WASI package reclaim-guard preparation is not a directory: ${preparationPath}`,
      );
    }

    const owner = await readStageWasiPackageLockOwner(path.join(preparationPath, 'owner.json'));
    const ownerIsValid =
      owner?.version === 2 &&
      owner.pid === identity.pid &&
      owner.token === identity.token &&
      (identity.executionFingerprint === undefined ||
        sameExecutionIdentityFingerprint(owner, identity.executionFingerprint));
    if (ownerIsValid) {
      if (await stageWasiPackageLockOwnerIsLive(owner)) continue;
    } else if (await stageWasiPackagePreparationIsLive(identity)) {
      continue;
    }

    await retireStageWasiPackageReclaimPath(preparationPath, transactionRoot, {
      afterReclaimGuardRetire,
      beforeReclaimGuardRetire,
    });
  }
}

async function stageWasiPackagePreparationIsLive(identity) {
  if (identity.executionFingerprint === undefined) return true;
  const scope = await compareExecutionIdentityFingerprintScope(identity.executionFingerprint);
  if (scope === 'non-local') return true;
  if (scope === 'previous-boot') return false;
  if (!processExists(identity.pid)) return false;
  if (identity.executionFingerprint.incarnation === unavailableExecutionIdentity) return true;
  const incarnation =
    identity.pid === process.pid
      ? await getCurrentProcessIncarnation()
      : await readProcessIncarnation(identity.pid);
  return incarnation === undefined
    ? true
    : executionIdentityFingerprintComponent(incarnation) ===
        identity.executionFingerprint.incarnation;
}

async function readStageWasiPackageReclaimCandidates(
  transactionRoot,
  { afterReclaimGuardRetire, beforeReclaimGuardRetire } = {},
) {
  const candidates = [];
  for (const entry of await readdir(transactionRoot)) {
    if (!entry.startsWith(transactionReclaimCandidatePrefix)) continue;
    const identity = parseStageWasiPackageReclaimCandidate(entry);
    if (!identity) {
      throw new Error(`Invalid WASI package reclaim-guard candidate name: ${entry}`);
    }
    const candidatePath = path.join(transactionRoot, entry);
    const stats = await lstatIfExists(candidatePath);
    if (!stats) continue;
    if (!stats.isDirectory()) {
      throw new Error(`WASI package reclaim-guard candidate is not a directory: ${candidatePath}`);
    }

    const owner = await readStageWasiPackageLockOwner(path.join(candidatePath, 'owner.json'));
    const ownerIsValid =
      stageWasiPackageLockOwnerIsValid(owner) &&
      owner.pid === identity.pid &&
      owner.token === identity.token;
    if (ownerIsValid && (await stageWasiPackageLockOwnerIsLive(owner))) {
      candidates.push({
        name: entry,
        owner,
        ticket: await readStageWasiPackageReclaimTicket(
          path.join(candidatePath, transactionReclaimTicketName),
        ),
      });
      continue;
    }

    if (!ownerIsValid) {
      candidates.push({ name: entry, owner: undefined, ticket: undefined });
      continue;
    }

    await retireStageWasiPackageReclaimPath(candidatePath, transactionRoot, {
      afterReclaimGuardRetire,
      beforeReclaimGuardRetire,
    });
  }
  return candidates;
}

async function reclaimDeadStageWasiPackageReclaimCandidates(
  transactionRoot,
  { afterReclaimGuardRetire, beforeReclaimGuardRetire } = {},
) {
  await readStageWasiPackageReclaimCandidates(transactionRoot, {
    afterReclaimGuardRetire,
    beforeReclaimGuardRetire,
  });
}

function parseStageWasiPackageReclaimCandidate(entry) {
  return parseStageWasiPackageReclaimIdentity(entry, transactionReclaimCandidatePrefix);
}

function parseStageWasiPackageLockPreparation(entry) {
  return parseStageWasiPackageVersionedPreparation(
    entry,
    transactionLockPreparationPrefix,
    transactionLockPreparationVersion,
  );
}

function parseStageWasiPackageReclaimPreparation(entry) {
  const identity = entry.slice(transactionReclaimPreparationPrefix.length);
  const versionPrefix = `v${transactionReclaimPreparationVersion}.`;
  if (identity.startsWith(versionPrefix)) {
    return parseStageWasiPackageVersionedPreparation(
      entry,
      transactionReclaimPreparationPrefix,
      transactionReclaimPreparationVersion,
    );
  }
  if (identity.startsWith('v1.')) {
    const fields = identity.slice('v1.'.length).split('.');
    if (fields.length !== 3) return undefined;
    const [pidText, , token] = fields;
    if (!/^[1-9]\d*$/.test(pidText) || token.length === 0) return undefined;
    const pid = Number(pidText);
    return Number.isSafeInteger(pid) ? { executionFingerprint: undefined, pid, token } : undefined;
  }
  const legacyIdentity = parseStageWasiPackageReclaimIdentity(
    entry,
    transactionReclaimPreparationPrefix,
  );
  return legacyIdentity ? { ...legacyIdentity, executionFingerprint: undefined } : undefined;
}

function parseStageWasiPackageVersionedPreparation(entry, prefix, version) {
  const identity = entry.slice(prefix.length);
  const versionPrefix = `v${version}.`;
  if (!identity.startsWith(versionPrefix)) return undefined;
  const fields = identity.slice(versionPrefix.length).split('.');
  if (fields.length !== 6) return undefined;
  const [pidText, machineId, bootId, pidNamespaceId, incarnation, token] = fields;
  if (
    !/^[1-9]\d*$/.test(pidText) ||
    !executionIdentityFingerprintIsValid({ bootId, incarnation, machineId, pidNamespaceId }) ||
    token.length === 0
  ) {
    return undefined;
  }
  const pid = Number(pidText);
  if (!Number.isSafeInteger(pid)) return undefined;

  return {
    executionFingerprint: { bootId, incarnation, machineId, pidNamespaceId },
    pid,
    token,
  };
}

function parseStageWasiPackageReclaimIdentity(entry, prefix) {
  const identity = entry.slice(prefix.length);
  const separator = identity.indexOf('.');
  if (separator === -1) return undefined;
  const pidText = identity.slice(0, separator);
  const token = identity.slice(separator + 1);
  if (!/^[1-9]\d*$/.test(pidText) || token.length === 0) return undefined;
  const pid = Number(pidText);
  return Number.isSafeInteger(pid) ? { pid, token } : undefined;
}

function stageWasiPackageReclaimPreparationName(pid, executionIdentity, token) {
  return stageWasiPackageVersionedPreparationName(
    transactionReclaimPreparationPrefix,
    transactionReclaimPreparationVersion,
    pid,
    executionIdentity,
    token,
  );
}

function stageWasiPackageLockPreparationName(pid, executionIdentity, token) {
  return stageWasiPackageVersionedPreparationName(
    transactionLockPreparationPrefix,
    transactionLockPreparationVersion,
    pid,
    executionIdentity,
    token,
  );
}

function stageWasiPackageVersionedPreparationName(prefix, version, pid, executionIdentity, token) {
  const fingerprint = executionIdentityFingerprint(executionIdentity);
  return `${prefix}v${version}.${pid}.${fingerprint.machineId}.${fingerprint.bootId}.${fingerprint.pidNamespaceId}.${fingerprint.incarnation}.${token}`;
}

function executionIdentityFingerprint(executionIdentity) {
  return {
    bootId: executionIdentityFingerprintComponent(executionIdentity.bootId),
    incarnation: executionIdentityFingerprintComponent(executionIdentity.incarnation),
    machineId: executionIdentityFingerprintComponent(executionIdentity.machineId),
    pidNamespaceId: executionIdentityFingerprintComponent(executionIdentity.pidNamespaceId),
  };
}

function executionIdentityFingerprintComponent(value) {
  return value === unavailableExecutionIdentity
    ? unavailableExecutionIdentity
    : createHash('sha256').update(value).digest('base64url').slice(0, 22);
}

function executionIdentityFingerprintIsValid(fingerprint) {
  return Boolean(
    fingerprint &&
    ['bootId', 'incarnation', 'machineId', 'pidNamespaceId'].every((field) =>
      /^(?:unavailable|[A-Za-z0-9_-]{22})$/.test(fingerprint[field]),
    ),
  );
}

async function readStageWasiPackageReclaimTicket(candidate) {
  const value = await readBoundedJsonFileIfExists(
    candidate,
    'WASI package reclaim-guard ticket',
    transactionReclaimTicketMaximumBytes,
  );
  if (value === undefined) return undefined;
  if (value?.version !== 1 || !Number.isSafeInteger(value.ticket) || value.ticket <= 0) {
    throw new Error(`Invalid WASI package reclaim-guard ticket: ${candidate}`);
  }
  return value.ticket;
}

async function recoverInterruptedDirectoryTransaction(transactionRoot) {
  const journalRoot = transactionJournalPath(transactionRoot);
  const journalStats = await lstatIfExists(journalRoot);
  if (!journalStats) return;
  if (!journalStats.isDirectory()) {
    throw new Error(`WASI package transaction journal is not a directory: ${journalRoot}`);
  }

  const state = await readBoundedJsonFileIfExists(
    path.join(journalRoot, transactionStateName),
    'WASI package transaction state',
    transactionStateMaximumBytes,
  );
  if (!state) {
    await assertUninitializedTransactionJournal(journalRoot);
    await removeTransactionJournal(transactionRoot);
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
    throw aggregateErrors(
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
          await renameDurably(replacement.destination, replacement.staged);
        }
        await renameDurably(backup, replacement.destination);
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

async function assertUninitializedTransactionJournal(journalRoot) {
  const entries = await readdir(journalRoot);
  if (entries.some((entry) => entry !== 'backups')) {
    throw new Error(`Invalid incomplete WASI package transaction journal at ${journalRoot}`);
  }
  const backupRoot = path.join(journalRoot, 'backups');
  const backupStats = await lstatIfExists(backupRoot);
  if (!backupStats) return;
  if (!backupStats.isDirectory() || (await readdir(backupRoot)).length > 0) {
    throw new Error(`Invalid incomplete WASI package transaction journal at ${journalRoot}`);
  }
}

async function removeTransactionJournal(transactionRoot, { afterOperation } = {}) {
  const journalRoot = transactionJournalPath(transactionRoot);
  await rm(path.join(journalRoot, 'backups'), { force: true, recursive: true });
  await syncDirectory(journalRoot);
  await afterOperation?.('cleanup-backups', -1);
  await rm(path.join(journalRoot, transactionStateName), { force: true });
  await syncDirectory(journalRoot);
  await afterOperation?.('cleanup-state', -1);
  await rm(journalRoot, { force: true, recursive: true });
  await syncDirectory(transactionRoot);
  await afterOperation?.('cleanup-journal', -1);
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

async function createCooperativeDirectory(candidate) {
  await mkdir(candidate, { mode: transactionDirectoryMode });
  if (process.platform !== 'win32') await chmod(candidate, transactionDirectoryMode);
}

async function renameDurably(source, destination) {
  await rename(source, destination);
  const destinationParent = path.dirname(destination);
  const sourceParent = path.dirname(source);
  await syncDirectory(destinationParent);
  if (sourceParent !== destinationParent) await syncDirectory(sourceParent);
}

async function syncDirectory(directory) {
  if (process.platform === 'win32') return;
  const handle = await open(directory, fsConstants.O_RDONLY);
  try {
    await handle.sync();
  } finally {
    await handle.close();
  }
}

function aggregateErrors(errors, message) {
  return new AggregateError(errors, message, { cause: errors[0] });
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

async function readBoundedJsonFileIfExists(candidate, label, maximumBytes) {
  let pathStats;
  try {
    pathStats = await lstat(candidate);
  } catch (error) {
    if (isNodeError(error) && error.code === 'ENOENT') return undefined;
    throw error;
  }
  if (!pathStats.isFile()) {
    throw new Error(`${label} is not a regular file: ${candidate}`);
  }

  let handle;
  try {
    const flags =
      fsConstants.O_RDONLY | (fsConstants.O_NOFOLLOW ?? 0) | (fsConstants.O_NONBLOCK ?? 0);
    handle = await open(candidate, flags);
  } catch (error) {
    if (isNodeError(error) && error.code === 'ENOENT') return undefined;
    throw new Error(`${label} cannot be opened safely: ${candidate}`, { cause: error });
  }

  try {
    const stats = await handle.stat();
    if (!stats.isFile() || stats.dev !== pathStats.dev || stats.ino !== pathStats.ino) {
      throw new Error(`${label} is not a regular file: ${candidate}`);
    }
    if (stats.size > maximumBytes) {
      throw new Error(`${label} exceeds ${maximumBytes} bytes: ${candidate}`);
    }

    const chunks = [];
    let total = 0;
    while (total <= maximumBytes) {
      const chunk = Buffer.allocUnsafe(Math.min(8 * 1024, maximumBytes + 1 - total));
      const { bytesRead } = await handle.read(chunk, 0, chunk.length, total);
      if (bytesRead === 0) break;
      chunks.push(chunk.subarray(0, bytesRead));
      total += bytesRead;
    }
    if (total > maximumBytes) {
      throw new Error(`${label} exceeds ${maximumBytes} bytes: ${candidate}`);
    }

    try {
      return JSON.parse(Buffer.concat(chunks, total).toString('utf8'));
    } catch (error) {
      throw new Error(`${label} contains invalid JSON: ${candidate}`, { cause: error });
    }
  } finally {
    await handle.close();
  }
}

async function readStageWasiPackageLockOwner(candidate, { required = false } = {}) {
  const owner = await readBoundedJsonFileIfExists(
    candidate,
    'WASI package lock owner',
    transactionLockOwnerMaximumBytes,
  );
  if (owner === undefined) {
    if (required) throw new Error(`WASI package lock owner is missing: ${candidate}`);
    return undefined;
  }
  if (!stageWasiPackageLockOwnerIsValid(owner)) {
    throw new Error(`Invalid WASI package lock owner: ${candidate}`);
  }
  return owner;
}

async function writeJsonAtomic(destination, value) {
  const temporary = path.join(
    path.dirname(destination),
    `.${path.basename(destination)}.${process.pid}.${randomUUID()}.tmp`,
  );
  let created = false;
  let renamed = false;
  try {
    const handle = await open(temporary, 'wx', transactionMetadataMode);
    created = true;
    try {
      if (process.platform !== 'win32') await handle.chmod(transactionMetadataMode);
      await handle.writeFile(`${JSON.stringify(value)}\n`, 'utf8');
      await handle.sync();
    } finally {
      await handle.close();
    }
    await rename(temporary, destination);
    renamed = true;
    await syncDirectory(path.dirname(destination));
  } finally {
    if (created && !renamed) {
      await rm(temporary, { force: true });
      await syncDirectory(path.dirname(destination));
    }
  }
}

async function stageWasiPackageLockOwnerIsLive(owner) {
  if (owner.version === 1) return true;
  const scope = await compareExecutionIdentityScope(owner);
  if (scope === 'non-local') return true;
  if (scope === 'previous-boot') return false;
  if (!processExists(owner.pid)) return false;
  if (owner.incarnation === unavailableExecutionIdentity) return true;
  const expectedFormat = processIncarnationFormat(owner.incarnation);
  if (expectedFormat === undefined) return true;

  const cacheKey = `${owner.pid}\0${owner.incarnation}`;
  const cached = processIncarnationMatchCache.get(cacheKey);
  if (cached && cached.expiresAt > Date.now()) {
    return (await cached.promise) ?? true;
  }

  const entry = {
    expiresAt: Number.POSITIVE_INFINITY,
    promise: processIncarnationMatches(owner.pid, owner.incarnation, expectedFormat),
  };
  processIncarnationMatchCache.set(cacheKey, entry);
  const matches = await entry.promise;
  if (processIncarnationMatchCache.get(cacheKey) === entry) {
    if (matches === true) {
      entry.expiresAt = Date.now() + processIncarnationMatchCacheMs;
    } else {
      processIncarnationMatchCache.delete(cacheKey);
    }
  }
  return matches ?? true;
}

async function compareExecutionIdentityScope(executionIdentity) {
  const current = await getCurrentExecutionIdentity();
  if (
    executionIdentity.machineId === unavailableExecutionIdentity ||
    executionIdentity.pidNamespaceId === unavailableExecutionIdentity ||
    current.machineId === unavailableExecutionIdentity ||
    current.pidNamespaceId === unavailableExecutionIdentity ||
    executionIdentity.machineId !== current.machineId ||
    executionIdentity.pidNamespaceId !== current.pidNamespaceId
  ) {
    return 'non-local';
  }
  if (
    executionIdentity.bootId !== unavailableExecutionIdentity &&
    current.bootId !== unavailableExecutionIdentity &&
    executionIdentity.bootId !== current.bootId
  ) {
    return 'previous-boot';
  }
  return 'local';
}

async function compareExecutionIdentityFingerprintScope(fingerprint) {
  const current = executionIdentityFingerprint(await getCurrentExecutionIdentity());
  if (
    fingerprint.machineId === unavailableExecutionIdentity ||
    fingerprint.pidNamespaceId === unavailableExecutionIdentity ||
    current.machineId === unavailableExecutionIdentity ||
    current.pidNamespaceId === unavailableExecutionIdentity ||
    fingerprint.machineId !== current.machineId ||
    fingerprint.pidNamespaceId !== current.pidNamespaceId
  ) {
    return 'non-local';
  }
  if (
    fingerprint.bootId !== unavailableExecutionIdentity &&
    current.bootId !== unavailableExecutionIdentity &&
    fingerprint.bootId !== current.bootId
  ) {
    return 'previous-boot';
  }
  return 'local';
}

function sameExecutionIdentityFingerprint(owner, fingerprint) {
  return Object.entries(executionIdentityFingerprint(owner)).every(
    ([field, value]) => fingerprint[field] === value,
  );
}

async function processIncarnationMatches(pid, expected, expectedFormat) {
  const incarnation =
    pid === process.pid ? await getCurrentProcessIncarnation() : await readProcessIncarnation(pid);
  if (incarnation === undefined || processIncarnationFormat(incarnation) !== expectedFormat) {
    return undefined;
  }
  return incarnation === expected;
}

function processIncarnationFormat(incarnation) {
  if (
    /^linux:[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}:\d+$/i.test(incarnation)
  ) {
    return 'linux';
  }
  if (/^win32:\d+$/.test(incarnation)) return 'win32';
  if (
    /^posix:(?:Sun|Mon|Tue|Wed|Thu|Fri|Sat) (?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) (?:[1-9]|[12]\d|3[01]) \d{2}:\d{2}:\d{2} \d{4}$/.test(
      incarnation,
    )
  ) {
    return 'posix';
  }
  return undefined;
}

async function getCurrentExecutionIdentity() {
  const pending = (currentExecutionIdentityPromise ??= readCurrentExecutionIdentity());
  const identity = await pending;
  if (
    Object.values(identity).includes(unavailableExecutionIdentity) &&
    currentExecutionIdentityPromise === pending
  ) {
    currentExecutionIdentityPromise = undefined;
  }
  return identity;
}

async function readCurrentExecutionIdentity() {
  const [machineId, bootId, pidNamespaceId, incarnation] = await Promise.all([
    readMachineIdentity(),
    readBootIdentity(),
    readPidNamespaceIdentity(),
    getCurrentProcessIncarnation(),
  ]);
  return {
    bootId: normalizeExecutionIdentityComponent(bootId),
    incarnation: normalizeExecutionIdentityComponent(incarnation),
    machineId: normalizeExecutionIdentityComponent(machineId),
    pidNamespaceId: normalizeExecutionIdentityComponent(pidNamespaceId),
  };
}

function normalizeExecutionIdentityComponent(value) {
  return isExecutionIdentityComponent(value, { allowUnavailable: false })
    ? value
    : unavailableExecutionIdentity;
}

async function readMachineIdentity() {
  if (process.platform === 'linux') {
    for (const candidate of ['/etc/machine-id', '/var/lib/dbus/machine-id']) {
      try {
        const value = (await readFile(candidate, 'utf8')).trim();
        if (/^[0-9a-f]{32}$/i.test(value)) return `linux-machine:${value.toLowerCase()}`;
      } catch {
        // Try the next stable machine-identity source.
      }
    }
  }
  if (process.platform === 'darwin') {
    try {
      const { stdout } = await execFileAsync(
        '/usr/sbin/ioreg',
        ['-rd1', '-c', 'IOPlatformExpertDevice'],
        {
          encoding: 'utf8',
          timeout: processIncarnationProbeTimeoutMs,
        },
      );
      const value = stdout.match(/"IOPlatformUUID"\s*=\s*"([0-9a-f-]{36})"/i)?.[1];
      if (value) return `darwin-machine:${value.toLowerCase()}`;
    } catch {
      return undefined;
    }
  }
  if (process.platform === 'win32') {
    try {
      const { stdout } = await execFileAsync(
        windowsPowerShellExecutable(),
        [
          '-NoLogo',
          '-NoProfile',
          '-NonInteractive',
          '-Command',
          "[Console]::Out.Write((Get-ItemProperty 'HKLM:\\SOFTWARE\\Microsoft\\Cryptography').MachineGuid)",
        ],
        {
          encoding: 'utf8',
          timeout: processIncarnationProbeTimeoutMs,
          windowsHide: true,
        },
      );
      const value = stdout.trim();
      if (/^[0-9a-f-]{36}$/i.test(value)) return `win32-machine:${value.toLowerCase()}`;
    } catch {
      return undefined;
    }
  }
  return undefined;
}

async function readBootIdentity() {
  if (process.platform === 'linux') {
    try {
      const value = (await readFile('/proc/sys/kernel/random/boot_id', 'utf8')).trim();
      return /^[0-9a-f-]{36}$/i.test(value) ? `linux-boot:${value.toLowerCase()}` : undefined;
    } catch {
      return undefined;
    }
  }
  if (process.platform === 'darwin') {
    try {
      const { stdout } = await execFileAsync('/usr/sbin/sysctl', ['-n', 'kern.bootsessionuuid'], {
        encoding: 'utf8',
        timeout: processIncarnationProbeTimeoutMs,
      });
      const value = stdout.trim();
      return /^[0-9a-f-]{36}$/i.test(value) ? `darwin-boot:${value.toLowerCase()}` : undefined;
    } catch {
      return undefined;
    }
  }
  if (process.platform === 'win32') {
    try {
      const { stdout } = await execFileAsync(
        windowsPowerShellExecutable(),
        [
          '-NoLogo',
          '-NoProfile',
          '-NonInteractive',
          '-Command',
          '[Console]::Out.Write((Get-CimInstance Win32_OperatingSystem).LastBootUpTime.ToUniversalTime().Ticks)',
        ],
        {
          encoding: 'utf8',
          timeout: processIncarnationProbeTimeoutMs,
          windowsHide: true,
        },
      );
      const ticks = stdout.trim();
      return /^\d+$/.test(ticks) ? `win32-boot:${ticks}` : undefined;
    } catch {
      return undefined;
    }
  }
  return undefined;
}

async function readPidNamespaceIdentity() {
  if (process.platform !== 'linux') return 'host-pid-namespace';
  try {
    const namespace = await readlink('/proc/self/ns/pid');
    return /^pid:\[\d+\]$/.test(namespace) ? `linux-${namespace}` : undefined;
  } catch {
    return undefined;
  }
}

async function getCurrentProcessIncarnation() {
  const pending = (currentProcessIncarnationPromise ??= readProcessIncarnation(process.pid));
  const incarnation = await pending;
  if (incarnation === undefined && currentProcessIncarnationPromise === pending) {
    currentProcessIncarnationPromise = undefined;
  }
  return incarnation;
}

async function readProcessIncarnation(pid) {
  if (process.platform === 'linux') {
    return readLinuxProcessIncarnation(pid);
  }
  if (process.platform === 'win32') {
    return readWindowsProcessIncarnation(pid);
  }
  return readPosixProcessIncarnation(pid);
}

async function readLinuxProcessIncarnation(pid) {
  try {
    const [stat, bootIdText] = await Promise.all([
      readFile(`/proc/${pid}/stat`, 'utf8'),
      readFile('/proc/sys/kernel/random/boot_id', 'utf8'),
    ]);
    const commandEnd = stat.lastIndexOf(')');
    if (commandEnd === -1) return undefined;
    const fields = stat
      .slice(commandEnd + 1)
      .trim()
      .split(/\s+/);
    const startTime = fields[19];
    const bootId = bootIdText.trim();
    if (!/^\d+$/.test(startTime) || bootId.length === 0 || /\s/.test(bootId)) return undefined;
    return `linux:${bootId}:${startTime}`;
  } catch {
    return undefined;
  }
}

async function readWindowsProcessIncarnation(pid) {
  try {
    const { stdout } = await execFileAsync(
      windowsPowerShellExecutable(),
      [
        '-NoLogo',
        '-NoProfile',
        '-NonInteractive',
        '-Command',
        `$target = Get-Process -Id ${pid} -ErrorAction Stop; [Console]::Out.Write($target.StartTime.ToUniversalTime().Ticks)`,
      ],
      {
        encoding: 'utf8',
        timeout: processIncarnationProbeTimeoutMs,
        windowsHide: true,
      },
    );
    const startTime = stdout.trim();
    return /^\d+$/.test(startTime) ? `win32:${startTime}` : undefined;
  } catch {
    return undefined;
  }
}

function windowsPowerShellExecutable() {
  const systemRoot = process.env.SystemRoot;
  return typeof systemRoot === 'string' && systemRoot.length > 0
    ? path.join(systemRoot, 'System32', 'WindowsPowerShell', 'v1.0', 'powershell.exe')
    : 'powershell.exe';
}

async function readPosixProcessIncarnation(pid) {
  try {
    const { stdout } = await execFileAsync('ps', ['-p', String(pid), '-o', 'lstart='], {
      encoding: 'utf8',
      env: { ...process.env, LC_ALL: 'C', TZ: 'UTC0' },
      timeout: processIncarnationProbeTimeoutMs,
    });
    const startTime = stdout.trim().replace(/\s+/g, ' ');
    return startTime.length > 0 ? `posix:${startTime}` : undefined;
  } catch {
    return undefined;
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

  await withStageWasiPackageLock(packageRoot, async (canonicalPackageRoot) => {
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
