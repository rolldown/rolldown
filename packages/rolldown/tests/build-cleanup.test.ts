import { beforeEach, expect, test, vi } from 'vitest';

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
