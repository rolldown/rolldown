import { existsSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  close: vi.fn(),
  generate: vi.fn(),
  getCloseTerminalErrors: vi.fn(),
  hasRetryableBuildCleanup: vi.fn(),
  retryRolldownBuildCleanup: vi.fn(),
  rolldown: vi.fn(),
}));

vi.mock('../src/plugin/parallel-plugin', () => ({
  assertParallelPluginOptionsSupported: vi.fn(),
}));

vi.mock('../src/api/rolldown', () => ({
  rolldown: mocks.rolldown,
}));

vi.mock('../src/api/rolldown/rolldown-build', () => ({
  hasRetryableBuildCleanup: mocks.hasRetryableBuildCleanup,
  retryRolldownBuildCleanup: mocks.retryRolldownBuildCleanup,
}));

vi.mock('../src/runtime-lifecycle', () => ({
  getCloseTerminalErrors: mocks.getCloseTerminalErrors,
  throwCloseErrors(errors: unknown[], message: string) {
    if (errors.length === 1) throw errors[0];
    if (errors.length > 1) {
      throw new AggregateError(errors, message, { cause: errors[0] });
    }
  },
}));

// @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
import { build } from '../src/api/build';
// @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
import {
  getRetryableCleanup,
  recoverRetryableCleanups,
  retryCleanupFromError,
} from '../src/utils/retryable-cleanup';
// @ts-ignore This focused unit test intentionally reaches package build tooling outside the test rootDir.
import {
  beginBuildArtifactTransaction,
  BINDING_BUILD_ARTIFACT_SELECTION,
} from '../build-binding-artifacts';

const temporaryDirectories: string[] = [];

beforeEach(() => {
  mocks.close.mockReset();
  mocks.generate.mockReset().mockResolvedValue({ output: [] });
  mocks.getCloseTerminalErrors.mockReset().mockReturnValue([]);
  mocks.hasRetryableBuildCleanup.mockReset();
  mocks.retryRolldownBuildCleanup.mockReset();
  mocks.rolldown.mockReset().mockResolvedValue({
    close: mocks.close,
    generate: mocks.generate,
    write: vi.fn(),
  });
});

afterEach(() => {
  for (const directory of temporaryDirectories.splice(0)) {
    rmSync(directory, { force: true, recursive: true });
  }
});

test('build surfaces terminal diagnostics after recovering a transport failure', async () => {
  const transportError = new Error('native close transport rejected');
  const terminalError = new Error('closeBundle failed after transport retry');
  let ownsResources = true;
  mocks.close.mockRejectedValueOnce(transportError);
  mocks.retryRolldownBuildCleanup.mockImplementationOnce(async () => {
    ownsResources = false;
    return [terminalError];
  });
  mocks.hasRetryableBuildCleanup.mockImplementation(() => ownsResources);

  const error = await build({ input: 'entry.js', write: false }).catch((error: unknown) => error);

  expect(error).toBe(terminalError);
  expect(mocks.close).toHaveBeenCalledOnce();
  expect(mocks.retryRolldownBuildCleanup).toHaveBeenCalledOnce();
  expect(getRetryableCleanup(error)).toBeUndefined();
});

test('build resolves after its immediate cleanup retry releases ownership', async () => {
  const cleanupError = new Error('runtime release failed');
  let ownsResources = true;
  mocks.close.mockRejectedValueOnce(cleanupError);
  mocks.retryRolldownBuildCleanup.mockImplementationOnce(async () => {
    ownsResources = false;
    return [];
  });
  mocks.hasRetryableBuildCleanup.mockImplementation(() => ownsResources);

  await expect(build({ input: 'entry.js', write: false })).resolves.toEqual({ output: [] });

  expect(mocks.close).toHaveBeenCalledOnce();
  expect(mocks.retryRolldownBuildCleanup).toHaveBeenCalledOnce();
});

test('build retries owned cleanup without duplicating a terminal diagnostic', async () => {
  const terminalError = new Error('closeBundle failed');
  const cleanupError = new Error('runtime release failed');
  const firstCloseError = new AggregateError(
    [terminalError, cleanupError],
    'Bundle native close or runtime release failed',
    { cause: terminalError },
  );
  mocks.close.mockRejectedValueOnce(firstCloseError);
  mocks.getCloseTerminalErrors.mockImplementation((error) =>
    error === firstCloseError ? [terminalError] : [],
  );
  let ownsResources = true;
  mocks.retryRolldownBuildCleanup.mockImplementationOnce(async () => {
    ownsResources = false;
    return [terminalError];
  });
  mocks.hasRetryableBuildCleanup.mockImplementation(() => ownsResources);

  const error = await build({ input: 'entry.js', write: false }).catch((error: unknown) => error);

  expect(error).toBe(terminalError);
  expect((error as Error).message).not.toContain('cleanup and retry both failed');
  expect(mocks.close).toHaveBeenCalledOnce();
  expect(mocks.retryRolldownBuildCleanup).toHaveBeenCalledOnce();
});

test('build deduplicates replayed terminal diagnostics by identity and multiplicity', async () => {
  const repeatedTerminalError = new Error('closeBundle failed twice');
  const newTerminalError = new Error('second closeBundle hook failed');
  const cleanupError = new Error('runtime release failed');
  const firstCloseError = new AggregateError(
    [repeatedTerminalError, repeatedTerminalError, cleanupError],
    'Bundle native close or runtime release failed',
    { cause: repeatedTerminalError },
  );
  mocks.close.mockRejectedValueOnce(firstCloseError);
  mocks.getCloseTerminalErrors.mockImplementation((error) =>
    error === firstCloseError ? [repeatedTerminalError, repeatedTerminalError] : [],
  );
  let ownsResources = true;
  mocks.retryRolldownBuildCleanup.mockImplementationOnce(async () => {
    ownsResources = false;
    return [repeatedTerminalError, repeatedTerminalError, newTerminalError];
  });
  mocks.hasRetryableBuildCleanup.mockImplementation(() => ownsResources);

  const error = await build({ input: 'entry.js', write: false }).catch((error: unknown) => error);

  expect(error).toBeInstanceOf(AggregateError);
  expect((error as AggregateError).errors).toEqual([
    repeatedTerminalError,
    repeatedTerminalError,
    newTerminalError,
  ]);
  expect((error as AggregateError).cause).toBe(repeatedTerminalError);
  expect(mocks.retryRolldownBuildCleanup).toHaveBeenCalledOnce();
  expect(getRetryableCleanup(error)).toBeUndefined();
});

test('build preserves terminal diagnostics when the final retry releases cleanup', async () => {
  const transportError = new Error('native close transport rejected');
  const terminalError = new Error('closeBundle failed after transport retry');
  const cleanupError = new Error('runtime release still failed');
  let ownsResources = true;
  mocks.close.mockRejectedValueOnce(transportError);
  mocks.retryRolldownBuildCleanup
    .mockRejectedValueOnce(cleanupError)
    .mockImplementationOnce(async () => {
      ownsResources = false;
      return [];
    });
  mocks.getCloseTerminalErrors.mockImplementation((error) =>
    error === cleanupError ? [terminalError] : [],
  );
  mocks.hasRetryableBuildCleanup.mockImplementation(() => ownsResources);

  const error = await build({ input: 'entry.js', write: false }).catch((error: unknown) => error);

  expect(error).toBe(terminalError);
  expect(mocks.retryRolldownBuildCleanup).toHaveBeenCalledTimes(2);
  expect(ownsResources).toBe(false);
  expect(getRetryableCleanup(error)).toBeUndefined();
});

test('build awaits final native close retry outside setup recovery', async () => {
  vi.useFakeTimers();
  try {
    const firstTransportError = new Error('first native close transport rejection');
    const secondTransportError = new Error('second native close transport rejection');
    let ownsResources = true;
    mocks.close.mockRejectedValueOnce(firstTransportError);
    mocks.retryRolldownBuildCleanup
      .mockRejectedValueOnce(secondTransportError)
      .mockImplementationOnce(async () => {
        ownsResources = false;
        return [];
      });
    mocks.hasRetryableBuildCleanup.mockImplementation(() => ownsResources);

    const operation = build({ input: 'entry.js', write: false });
    let settled = false;
    void operation.then(
      () => {
        settled = true;
      },
      () => {
        settled = true;
      },
    );
    await waitForCallCount(mocks.retryRolldownBuildCleanup, 1);

    await expect(recoverRetryableCleanups()).resolves.toBeUndefined();
    expect(mocks.close).toHaveBeenCalledOnce();
    expect(mocks.retryRolldownBuildCleanup).toHaveBeenCalledOnce();
    expect(settled).toBe(false);
    expect(vi.getTimerCount()).toBe(1);

    await vi.runOnlyPendingTimersAsync();
    const output = await operation;

    expect(output).toEqual({ output: [] });
    expect(mocks.close).toHaveBeenCalledOnce();
    expect(mocks.retryRolldownBuildCleanup).toHaveBeenCalledTimes(2);
    expect(ownsResources).toBe(false);
    expect(getRetryableCleanup(firstTransportError)).toBeUndefined();
    expect(vi.getTimerCount()).toBe(0);
  } finally {
    vi.useRealTimers();
  }
});

test('build surfaces terminal diagnostics from its awaited final retry', async () => {
  vi.useFakeTimers();
  try {
    const firstTransportError = new Error('first native close transport rejection');
    const secondTransportError = new Error('second native close transport rejection');
    const terminalError = new Error('closeBundle failed during final cleanup');
    let ownsResources = true;
    mocks.close.mockRejectedValueOnce(firstTransportError);
    mocks.retryRolldownBuildCleanup
      .mockRejectedValueOnce(secondTransportError)
      .mockImplementationOnce(async () => {
        ownsResources = false;
        return [terminalError];
      });
    mocks.hasRetryableBuildCleanup.mockImplementation(() => ownsResources);

    const operation = build({ input: 'entry.js', write: false });
    let settled = false;
    void operation.then(
      () => {
        settled = true;
      },
      () => {
        settled = true;
      },
    );
    await waitForCallCount(mocks.retryRolldownBuildCleanup, 1);

    expect(settled).toBe(false);
    await vi.runOnlyPendingTimersAsync();
    const error = await operation.catch((error: unknown) => error);

    expect(mocks.retryRolldownBuildCleanup).toHaveBeenCalledTimes(2);
    expect(error).toBe(terminalError);
    expect(getRetryableCleanup(error)).toBeUndefined();
    expect(vi.getTimerCount()).toBe(0);
  } finally {
    vi.useRealTimers();
  }
});

test('build bounds a persistent final cleanup failure and retains explicit ownership', async () => {
  vi.useFakeTimers();
  try {
    const firstTransportError = new Error('first native close transport rejection');
    const secondTransportError = new Error('second native close transport rejection');
    const finalTransportError = new Error('final native close transport rejection');
    mocks.close.mockRejectedValueOnce(firstTransportError);
    mocks.retryRolldownBuildCleanup
      .mockRejectedValueOnce(secondTransportError)
      .mockRejectedValueOnce(finalTransportError);
    mocks.hasRetryableBuildCleanup.mockReturnValue(true);

    const operation = build({ input: 'entry.js', write: false });
    const result = operation.catch((error: unknown) => error);
    await waitForCallCount(mocks.retryRolldownBuildCleanup, 1);
    await waitForTimerCount(1);

    await vi.runOnlyPendingTimersAsync();
    const error = await result;

    expect(error).toMatchObject({
      cause: firstTransportError,
      errors: [firstTransportError, secondTransportError, finalTransportError],
    });
    expect(mocks.retryRolldownBuildCleanup).toHaveBeenCalledTimes(2);
    expect(getRetryableCleanup(error)).toBeTypeOf('function');
    expect(vi.getTimerCount()).toBe(0);
  } finally {
    vi.useRealTimers();
  }
});

test('build and close failure keeps retryable cleanup on the top-level error', async () => {
  const buildError = new Error('generate failed');
  const closeError = new Error('native close transport rejected');
  const retryError = new Error('runtime release still failed');
  const finalRetryError = new Error('runtime release failed on final retry');
  let ownsResources = true;
  mocks.generate.mockRejectedValueOnce(buildError);
  mocks.close.mockRejectedValueOnce(closeError);
  mocks.retryRolldownBuildCleanup
    .mockRejectedValueOnce(retryError)
    .mockRejectedValueOnce(finalRetryError)
    .mockImplementationOnce(async () => {
      ownsResources = false;
      return [];
    });
  mocks.hasRetryableBuildCleanup.mockImplementation(() => ownsResources);

  const error = await build({ input: 'entry.js', write: false }).catch((error: unknown) => error);

  expect(error).toMatchObject({
    cause: buildError,
    errors: [
      buildError,
      {
        cause: closeError,
        errors: [closeError, retryError, finalRetryError],
      },
    ],
  });
  const nestedCloseError = (error as AggregateError).errors[1];
  expect(getRetryableCleanup(error)).toBeTypeOf('function');
  expect(getRetryableCleanup(nestedCloseError)).toBeUndefined();

  await expect(retryCleanupFromError(error, 'retry failed')).rejects.toBe(error);
  expect(mocks.retryRolldownBuildCleanup).toHaveBeenCalledTimes(3);
  expect(getRetryableCleanup(error)).toBeUndefined();
});

test('failed binding build removes only artifacts created by that invocation', () => {
  const directory = createTemporaryDirectory();
  const nativeArtifact = join(directory, 'rolldown-binding.darwin-arm64.node');
  const threadlessArtifact = join(directory, 'rolldown-binding.wasm32-wasip1.wasm');
  const threadedArtifact = join(directory, 'rolldown-binding.wasm32-wasi.wasm');
  const threadedDebugArtifact = join(directory, 'rolldown-binding.wasm32-wasi.debug.wasm');
  const browserEntry = join(directory, 'browser.js');
  const unrelatedWasm = join(directory, 'unrelated.wasm');
  writeFileSync(nativeArtifact, 'native');
  writeFileSync(threadlessArtifact, 'threadless');
  const nativeInode = statSync(nativeArtifact).ino;
  const threadlessInode = statSync(threadlessArtifact).ino;

  const transaction = beginBuildArtifactTransaction(directory, BINDING_BUILD_ARTIFACT_SELECTION);
  writeFileSync(threadedArtifact, 'partial threaded build');
  writeFileSync(threadedDebugArtifact, 'partial threaded debug build');
  writeFileSync(browserEntry, 'partial browser entry');
  writeFileSync(unrelatedWasm, 'not owned by the binding build');

  transaction.rollback();

  expect(existsSync(threadedArtifact)).toBe(false);
  expect(existsSync(threadedDebugArtifact)).toBe(false);
  expect(existsSync(browserEntry)).toBe(false);
  expect(readFileSync(nativeArtifact, 'utf8')).toBe('native');
  expect(readFileSync(threadlessArtifact, 'utf8')).toBe('threadless');
  expect(statSync(nativeArtifact).ino).toBe(nativeInode);
  expect(statSync(threadlessArtifact).ino).toBe(threadlessInode);
  expect(readFileSync(unrelatedWasm, 'utf8')).toBe('not owned by the binding build');
});

test('failed binding build restores overwritten and deleted artifacts', () => {
  const directory = createTemporaryDirectory();
  const releaseArtifact = join(directory, 'rolldown-binding.wasm32-wasi.wasm');
  const debugArtifact = join(directory, 'rolldown-binding.wasm32-wasi.debug.wasm');
  writeFileSync(releaseArtifact, 'previous release');
  writeFileSync(debugArtifact, 'previous debug');

  const rootLoader = join(directory, 'binding.cjs');
  const wasiLoader = join(directory, 'rolldown-binding.wasip1.cjs');
  const newWorkerLoader = join(directory, 'wasi-worker.mjs');
  writeFileSync(rootLoader, 'previous root loader');
  writeFileSync(wasiLoader, 'previous WASI loader');

  const transaction = beginBuildArtifactTransaction(directory, BINDING_BUILD_ARTIFACT_SELECTION);
  writeFileSync(releaseArtifact, 'invalid replacement');
  rmSync(debugArtifact);
  writeFileSync(rootLoader, 'invalid root loader');
  rmSync(wasiLoader);
  writeFileSync(newWorkerLoader, 'partial worker loader');

  transaction.rollback();

  expect(readFileSync(releaseArtifact, 'utf8')).toBe('previous release');
  expect(readFileSync(debugArtifact, 'utf8')).toBe('previous debug');
  expect(readFileSync(rootLoader, 'utf8')).toBe('previous root loader');
  expect(readFileSync(wasiLoader, 'utf8')).toBe('previous WASI loader');
  expect(existsSync(newWorkerLoader)).toBe(false);
});

test('failed binding build restores browser.js after post-NAPI validation fails', () => {
  const directory = createTemporaryDirectory();
  const browserEntry = join(directory, 'browser.js');
  const postNapiValidationError = new Error('post-NAPI binding validation failed');
  writeFileSync(browserEntry, 'previous browser entry');

  const transaction = beginBuildArtifactTransaction(directory, BINDING_BUILD_ARTIFACT_SELECTION);
  let failure: unknown;
  try {
    writeFileSync(browserEntry, 'browser entry written by NAPI');
    throw postNapiValidationError;
  } catch (error) {
    failure = error;
    transaction.rollback();
  }

  expect(failure).toBe(postNapiValidationError);
  expect(readFileSync(browserEntry, 'utf8')).toBe('previous browser entry');
});

test('successful binding build retains replacement artifacts', () => {
  const directory = createTemporaryDirectory();
  const artifact = join(directory, 'rolldown-binding.darwin-arm64.node');
  writeFileSync(artifact, 'previous native');

  const transaction = beginBuildArtifactTransaction(directory, BINDING_BUILD_ARTIFACT_SELECTION);
  writeFileSync(artifact, 'new native');
  transaction.commit();

  expect(readFileSync(artifact, 'utf8')).toBe('new native');
});

function createTemporaryDirectory(): string {
  const directory = mkdtempSync(join(tmpdir(), 'rolldown-build-cleanup-'));
  temporaryDirectories.push(directory);
  return directory;
}

async function waitForCallCount(
  mock: { mock: { calls: unknown[][] } },
  expectedCount: number,
): Promise<void> {
  for (let attempt = 0; attempt < 100; attempt++) {
    if (mock.mock.calls.length >= expectedCount) return;
    await Promise.resolve();
  }
  throw new Error(`Expected ${expectedCount} calls, received ${mock.mock.calls.length}`);
}

async function waitForTimerCount(expectedCount: number): Promise<void> {
  for (let attempt = 0; attempt < 100; attempt++) {
    if (vi.getTimerCount() >= expectedCount) return;
    await Promise.resolve();
  }
  throw new Error(`Expected ${expectedCount} timers, received ${vi.getTimerCount()}`);
}
