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

import { NapiCli } from '@napi-rs/cli';
import { parse } from 'acorn';

const defaultRepoRoot = fileURLToPath(new URL('../../', import.meta.url));
const coreRuntimePackages = ['@emnapi/core', '@emnapi/runtime', '@napi-rs/wasm-runtime'];
const runtimePackages = [...coreRuntimePackages, 'buffer'];
const notices = ['LICENSE', 'THIRD-PARTY-LICENSE'];
const transactionLockName = '.stage-wasi-packages.lock';
const transactionLockCandidatePrefix = `${transactionLockName}.candidate.`;
const transactionLockRetiredPrefix = `${transactionLockName}.retired.`;
const transactionReclaimCandidatePrefix = `${transactionLockName}.reclaim.`;
const transactionReclaimTicketName = 'ticket.json';
const transactionJournalName = '.stage-wasi-packages.transaction';
const transactionStateName = 'state.json';
const stagingDirectoryPrefix = '.stage-wasi-packages-';
const transactionStateVersion = 1;
const transactionLockTimeoutMs = 60_000;
const incompleteLockGracePeriodMs = 5_000;
const transactionLockPollMs = 20;
const maximumTransactionReplacements = 64;
const napiCli = new NapiCli();

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

export async function withStageWasiPackageLock(
  transactionRoot,
  operation,
  {
    afterLockPublishFailure,
    afterLockCandidateCreate,
    afterLockRetire,
    afterReclaimGuardCandidateCreate,
    afterReclaimGuardTicketPublish,
    afterStaleLockRetire,
    afterStaleLockObserved,
    beforeLockPublish,
  } = {},
) {
  await mkdir(transactionRoot, { recursive: true });
  await assertDirectory(transactionRoot, 'WASI package transaction root');
  const canonicalRoot = await realpath(transactionRoot);
  const release = await acquireStageWasiPackageLock(canonicalRoot, {
    afterLockPublishFailure,
    afterLockCandidateCreate,
    afterLockRetire,
    afterReclaimGuardCandidateCreate,
    afterReclaimGuardTicketPublish,
    afterStaleLockRetire,
    afterStaleLockObserved,
    beforeLockPublish,
  });
  let operationError;
  let result;
  try {
    await removeRetiredStageWasiPackageLocks(canonicalRoot);
    await reclaimStaleStageWasiPackageLockCandidates(canonicalRoot);
    await reclaimDeadStageWasiPackageReclaimCandidates(canonicalRoot);
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

async function acquireStageWasiPackageLock(
  transactionRoot,
  {
    afterLockPublishFailure,
    afterLockCandidateCreate,
    afterLockRetire,
    afterReclaimGuardCandidateCreate,
    afterReclaimGuardTicketPublish,
    afterStaleLockRetire,
    afterStaleLockObserved,
    beforeLockPublish,
  } = {},
) {
  const lockPath = path.join(transactionRoot, transactionLockName);
  const ownerPath = path.join(lockPath, 'owner.json');
  const deadline = Date.now() + transactionLockTimeoutMs;

  while (true) {
    const token = randomUUID();
    const candidateLockPath = path.join(
      transactionRoot,
      `${transactionLockCandidatePrefix}${process.pid}.${token}`,
    );
    const candidateOwnerPath = path.join(candidateLockPath, 'owner.json');
    let publishingLockCandidate = false;
    try {
      await mkdir(candidateLockPath);
      await afterLockCandidateCreate?.(candidateLockPath);
      await writeJsonAtomic(candidateOwnerPath, {
        version: 1,
        createdAt: Date.now(),
        pid: process.pid,
        token,
      });
      await beforeLockPublish?.();
      publishingLockCandidate = true;
      await rename(candidateLockPath, lockPath);
    } catch (error) {
      await rm(candidateLockPath, { force: true, recursive: true });
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
          afterReclaimGuardTicketPublish,
          afterStaleLockRetire,
          afterStaleLockObserved,
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

    return async () => {
      const owner = await readJsonIfExists(ownerPath);
      if (!owner || owner.token !== token || owner.pid !== process.pid) {
        throw new Error(`Lost ownership of the WASI package transaction lock at ${lockPath}`);
      }
      const retiredPath = stageWasiPackageLockRetiredPath(transactionRoot);
      await rename(lockPath, retiredPath);
      let retireHookError;
      try {
        await afterLockRetire?.(retiredPath);
      } catch (error) {
        retireHookError = error;
      }
      try {
        await rm(retiredPath, { force: true, recursive: true });
      } catch (cleanupError) {
        if (retireHookError) {
          throw new AggregateError(
            [retireHookError, cleanupError],
            `WASI package transaction lock retirement failed at ${retiredPath}`,
          );
        }
        throw cleanupError;
      }
      if (retireHookError) throw retireHookError;
    };
  }
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
    afterReclaimGuardTicketPublish,
    afterStaleLockRetire,
    afterStaleLockObserved,
  } = {},
) {
  const lockStats = await lstatIfExists(lockPath);
  if (!lockStats) return true;
  if (!lockStats.isDirectory()) {
    throw new Error(`WASI package transaction lock is not a directory: ${lockPath}`);
  }

  const observedOwner = await readLockOwnerIfExists(ownerPath);
  if (!stageWasiPackageLockIsStale(lockStats, observedOwner)) return false;
  await afterStaleLockObserved?.({ owner: observedOwner });

  const releaseReclaimGuard = await acquireStageWasiPackageReclaimGuard(transactionRoot, {
    afterReclaimGuardCandidateCreate,
    afterReclaimGuardTicketPublish,
  });
  let retiredPath;
  try {
    const currentStats = await lstatIfExists(lockPath);
    if (!currentStats) return true;
    if (!currentStats.isDirectory()) {
      throw new Error(`WASI package transaction lock is not a directory: ${lockPath}`);
    }
    const currentOwner = await readLockOwnerIfExists(ownerPath);
    if (!sameStageWasiPackageLockOwner(currentOwner, observedOwner)) return false;
    if (!stageWasiPackageLockIsStale(currentStats, currentOwner)) return false;

    retiredPath = stageWasiPackageLockRetiredPath(path.dirname(lockPath));
    try {
      await rename(lockPath, retiredPath);
    } catch (error) {
      if (isNodeError(error) && error.code === 'ENOENT') return true;
      return false;
    }
    await afterStaleLockRetire?.(retiredPath);
  } finally {
    await releaseReclaimGuard();
  }
  if (retiredPath) await rm(retiredPath, { force: true, recursive: true });
  return true;
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

    const owner = await readLockOwnerIfExists(path.join(candidateLockPath, 'owner.json'));
    if (!stageWasiPackageLockOwnerIsValid(owner)) {
      const candidatePid = parseStageWasiPackageLockCandidatePid(entry);
      if (candidatePid !== undefined && processExists(candidatePid)) continue;
    }
    if (!stageWasiPackageLockIsStale(candidateStats, owner)) continue;

    const retiredPath = stageWasiPackageLockRetiredPath(transactionRoot);
    try {
      await rename(candidateLockPath, retiredPath);
    } catch (error) {
      if (isNodeError(error) && error.code === 'ENOENT') continue;
      throw error;
    }
    await rm(retiredPath, { force: true, recursive: true });
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
    await rm(retiredPath, { force: true, recursive: true });
  }
}

function stageWasiPackageLockRetiredPath(transactionRoot) {
  return path.join(
    transactionRoot,
    `${transactionLockRetiredPrefix}${process.pid}.${randomUUID()}`,
  );
}

function stageWasiPackageLockOwnerIsValid(owner) {
  return Boolean(
    owner &&
    owner.version === 1 &&
    Number.isSafeInteger(owner.createdAt) &&
    owner.createdAt > 0 &&
    Number.isSafeInteger(owner.pid) &&
    owner.pid > 0 &&
    typeof owner.token === 'string',
  );
}

function stageWasiPackageLockIsStale(lockStats, owner) {
  return stageWasiPackageLockOwnerIsValid(owner)
    ? !processExists(owner.pid)
    : Date.now() - lockStats.mtimeMs >= incompleteLockGracePeriodMs;
}

function parseStageWasiPackageLockCandidatePid(entry) {
  const pidText = entry.slice(transactionLockCandidatePrefix.length).split('.', 1)[0];
  if (!/^[1-9]\d*$/.test(pidText)) return undefined;
  const pid = Number(pidText);
  return Number.isSafeInteger(pid) ? pid : undefined;
}

function sameStageWasiPackageLockOwner(first, second) {
  if (!first || !second) return first === second;
  return (
    first.version === second.version &&
    first.createdAt === second.createdAt &&
    first.pid === second.pid &&
    first.token === second.token
  );
}

// Stale-lock reclaimers use unique Lamport bakery candidates, so crash cleanup
// never mutates a path that a successor can own. See
// internal-docs/async-runtime/implementation.md.
export async function acquireStageWasiPackageReclaimGuard(
  transactionRoot,
  {
    afterReclaimGuardCandidateCreate,
    afterReclaimGuardTicketPublish,
    beforeReclaimGuardAdmission,
    beforeReclaimGuardTicketPublish,
  } = {},
) {
  const token = randomUUID();
  const candidateName = `${transactionReclaimCandidatePrefix}${process.pid}.${token}`;
  const candidatePath = path.join(transactionRoot, candidateName);
  const ownerPath = path.join(candidatePath, 'owner.json');
  const ticketPath = path.join(candidatePath, transactionReclaimTicketName);
  const owner = {
    version: 1,
    createdAt: Date.now(),
    pid: process.pid,
    token,
  };
  const deadline = Date.now() + transactionLockTimeoutMs;
  let ticket;

  try {
    await mkdir(candidatePath);
    await afterReclaimGuardCandidateCreate?.(candidatePath);
    await writeJsonAtomic(ownerPath, owner);

    const choosingCandidates = await readStageWasiPackageReclaimCandidates(transactionRoot);
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
      const candidates = await readStageWasiPackageReclaimCandidates(transactionRoot);
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
    await rm(candidatePath, { force: true, recursive: true });
    throw error;
  }

  return async () => {
    const [currentOwner, currentTicket] = await Promise.all([
      readLockOwnerIfExists(ownerPath),
      readStageWasiPackageReclaimTicket(ticketPath),
    ]);
    if (!sameStageWasiPackageLockOwner(currentOwner, owner) || currentTicket !== ticket) {
      throw new Error(`Lost ownership of the WASI package reclaim guard at ${candidatePath}`);
    }
    await rm(candidatePath, { force: true, recursive: true });
  };
}

async function readStageWasiPackageReclaimCandidates(transactionRoot) {
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

    const owner = await readLockOwnerIfExists(path.join(candidatePath, 'owner.json'));
    const ownerIsValid =
      stageWasiPackageLockOwnerIsValid(owner) &&
      owner.pid === identity.pid &&
      owner.token === identity.token;
    const candidatePid = ownerIsValid ? owner.pid : identity.pid;
    if (!processExists(candidatePid)) {
      await rm(candidatePath, { force: true, recursive: true });
      continue;
    }
    if (!ownerIsValid) {
      candidates.push({ name: entry, owner: undefined, ticket: undefined });
      continue;
    }

    candidates.push({
      name: entry,
      owner,
      ticket: await readStageWasiPackageReclaimTicket(
        path.join(candidatePath, transactionReclaimTicketName),
      ),
    });
  }
  return candidates;
}

async function reclaimDeadStageWasiPackageReclaimCandidates(transactionRoot) {
  await readStageWasiPackageReclaimCandidates(transactionRoot);
}

function parseStageWasiPackageReclaimCandidate(entry) {
  const identity = entry.slice(transactionReclaimCandidatePrefix.length);
  const separator = identity.indexOf('.');
  if (separator === -1) return undefined;
  const pidText = identity.slice(0, separator);
  const token = identity.slice(separator + 1);
  if (!/^[1-9]\d*$/.test(pidText) || token.length === 0) return undefined;
  const pid = Number(pidText);
  return Number.isSafeInteger(pid) ? { pid, token } : undefined;
}

async function readStageWasiPackageReclaimTicket(candidate) {
  const value = await readJsonIfExists(candidate);
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
