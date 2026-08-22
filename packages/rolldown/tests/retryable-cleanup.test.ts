import { expect, test, vi } from 'vitest';
import {
  attachRetryableCleanup,
  getRetryableCleanup,
  recoverRetryableCleanups,
  retryCleanupFromError,
  runRetryableCleanup,
  trackRetryableCleanupOwnership,
} from '../src/utils/retryable-cleanup';

test('abandoned recovery excludes direct-only cleanup', async () => {
  const cleanup = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
  trackRetryableCleanupOwnership(cleanup, () => true, {
    recoverAbandoned: false,
  });
  attachRetryableCleanup(new Error('direct retry owner'), cleanup);

  await expect(recoverRetryableCleanups()).resolves.toBeUndefined();
  expect(cleanup).not.toHaveBeenCalled();
});

test('ownership-free attachment retains no cleanup claim', () => {
  const cleanup = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
  trackRetryableCleanupOwnership(cleanup, () => false, {
    recoverAbandoned: false,
  });
  const error = new Error('terminal cleanup owner');

  attachRetryableCleanup(error, cleanup);

  expect(getRetryableCleanup(error)).toBeUndefined();
  expect(cleanup).not.toHaveBeenCalled();
});

test('lookup clears an abandoned cleanup after ownership disappears', async () => {
  let ownsResources = true;
  const cleanup = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
  trackRetryableCleanupOwnership(cleanup, () => ownsResources);
  const error = new Error('stale cleanup owner');
  attachRetryableCleanup(error, cleanup);

  ownsResources = false;
  expect(getRetryableCleanup(error)).toBeUndefined();

  await expect(recoverRetryableCleanups()).resolves.toBeUndefined();
  expect(cleanup).not.toHaveBeenCalled();
});

test('abandoned recovery skips cleanup whose ownership disappeared', async () => {
  let ownsResources = true;
  const cleanup = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
  trackRetryableCleanupOwnership(cleanup, () => ownsResources);
  attachRetryableCleanup(new Error('abandoned setup owner'), cleanup);

  ownsResources = false;
  await expect(recoverRetryableCleanups()).resolves.toBeUndefined();
  expect(cleanup).not.toHaveBeenCalled();
});

test('successful cleanup invalidates every error claim immediately', async () => {
  let ownsResources = true;
  const cleanup = vi.fn(async () => {
    ownsResources = false;
  });
  trackRetryableCleanupOwnership(cleanup, () => ownsResources);
  const firstError = new Error('first cleanup owner');
  const secondError = new Error('second cleanup owner');
  attachRetryableCleanup(firstError, cleanup);
  attachRetryableCleanup(secondError, cleanup);

  await expect(runRetryableCleanup(cleanup)).resolves.toBeUndefined();

  expect(getRetryableCleanup(firstError)).toBeUndefined();
  expect(getRetryableCleanup(secondError)).toBeUndefined();
  await expect(recoverRetryableCleanups()).resolves.toBeUndefined();
  expect(cleanup).toHaveBeenCalledOnce();
});

test('rejected attempt invalidates self-attachments after releasing ownership', async () => {
  const terminalError = new Error('terminal cleanup failure');
  let ownsResources = true;
  const cleanup = vi.fn(async () => {
    attachRetryableCleanup(terminalError, cleanup);
    ownsResources = false;
    throw terminalError;
  });
  trackRetryableCleanupOwnership(cleanup, () => ownsResources, {
    recoverAbandoned: false,
  });

  await expect(runRetryableCleanup(cleanup)).rejects.toBe(terminalError);

  expect(cleanup).toHaveBeenCalledOnce();
  expect(getRetryableCleanup(terminalError)).toBeUndefined();
});

test('non-retaining owned failure remains explicit and outside abandoned recovery', async () => {
  const cleanupError = new Error('nested cleanup failure');
  const cleanup = vi.fn(async () => {
    attachRetryableCleanup(cleanupError, cleanup);
    throw cleanupError;
  });
  trackRetryableCleanupOwnership(cleanup, () => true);

  await expect(runRetryableCleanup(cleanup, false)).rejects.toBe(cleanupError);

  expect(getRetryableCleanup(cleanupError)).toBe(cleanup);
  await expect(recoverRetryableCleanups()).resolves.toBeUndefined();
  expect(cleanup).toHaveBeenCalledOnce();
});

test('terminal explicit retry invalidates the original and aggregate claims', async () => {
  const originalError = new Error('original cleanup failure');
  const terminalError = new Error('terminal retry failure');
  let ownsResources = true;
  const cleanup = vi.fn(async () => {
    ownsResources = false;
    throw terminalError;
  });
  trackRetryableCleanupOwnership(cleanup, () => ownsResources, {
    recoverAbandoned: false,
  });
  attachRetryableCleanup(originalError, cleanup);

  const error = await retryCleanupFromError(originalError, 'cleanup retry failed').catch(
    (error: unknown) => error,
  );

  expect(error).toMatchObject({
    cause: originalError,
    errors: [originalError, terminalError],
  });
  expect(cleanup).toHaveBeenCalledOnce();
  expect(getRetryableCleanup(originalError)).toBeUndefined();
  expect(getRetryableCleanup(error)).toBeUndefined();
});

test('abandoned recovery preserves a rejection after ownership is released', async () => {
  const terminalError = new Error('terminal cleanup failure');
  let ownsResources = true;
  const cleanup = vi.fn(async () => {
    ownsResources = false;
    throw terminalError;
  });
  trackRetryableCleanupOwnership(cleanup, () => ownsResources);
  attachRetryableCleanup(new Error('abandoned setup owner'), cleanup);

  await expect(recoverRetryableCleanups()).rejects.toBe(terminalError);
  expect(cleanup).toHaveBeenCalledOnce();
});

test('shared recovery drains newly attached cleanup without retrying a persistent failure', async () => {
  let releaseFirstCleanup!: () => void;
  let firstCleanupStarted!: () => void;
  const firstCleanupStart = new Promise<void>((resolve) => {
    firstCleanupStarted = resolve;
  });
  const firstCleanupRelease = new Promise<void>((resolve) => {
    releaseFirstCleanup = resolve;
  });
  const persistentError = new Error('persistent cleanup failure');
  let firstOwnsResources = true;
  const firstOwner = new Error('first cleanup owner');
  const firstCleanup = vi.fn(async () => {
    firstCleanupStarted();
    await firstCleanupRelease;
    throw persistentError;
  });
  trackRetryableCleanupOwnership(firstCleanup, () => firstOwnsResources);
  attachRetryableCleanup(firstOwner, firstCleanup);

  const recovery = recoverRetryableCleanups();
  await firstCleanupStart;

  let secondOwnsResources = true;
  const secondOwner = new Error('second cleanup owner');
  const secondCleanup = vi.fn(async () => {
    secondOwnsResources = false;
  });
  trackRetryableCleanupOwnership(secondCleanup, () => secondOwnsResources);
  attachRetryableCleanup(secondOwner, secondCleanup);

  const concurrentRecovery = recoverRetryableCleanups();
  expect(concurrentRecovery).toBe(recovery);
  releaseFirstCleanup();

  const results = await Promise.allSettled([recovery, concurrentRecovery]);
  expect(results).toEqual([
    { status: 'rejected', reason: persistentError },
    { status: 'rejected', reason: persistentError },
  ]);
  expect(firstCleanup).toHaveBeenCalledOnce();
  expect(secondCleanup).toHaveBeenCalledOnce();
  expect(getRetryableCleanup(secondOwner)).toBeUndefined();

  firstOwnsResources = false;
  await expect(recoverRetryableCleanups()).resolves.toBeUndefined();
  expect(firstCleanup).toHaveBeenCalledOnce();
  expect(getRetryableCleanup(firstOwner)).toBeUndefined();
});

test('recovery includes an attachment made before its shared promise settles', async () => {
  const recovery = recoverRetryableCleanups();
  let ownsResources = true;
  const cleanup = vi.fn(async () => {
    ownsResources = false;
  });
  trackRetryableCleanupOwnership(cleanup, () => ownsResources);
  attachRetryableCleanup(new Error('late cleanup owner'), cleanup);

  const joinedRecovery = recoverRetryableCleanups();
  expect(joinedRecovery).toBe(recovery);
  try {
    await expect(joinedRecovery).resolves.toBeUndefined();
    expect(cleanup).toHaveBeenCalledOnce();
  } finally {
    ownsResources = false;
    await recoverRetryableCleanups().catch(() => {});
  }
});

test('synchronous cleanup self-reentry is an idempotent no-op', async () => {
  let nestedAttempt!: Promise<void>;
  const release = vi.fn();
  const cleanup = vi.fn(() => {
    release();
    nestedAttempt = runRetryableCleanup(cleanup);
    return nestedAttempt;
  });

  const attempt = runRetryableCleanup(cleanup, false);
  await expect(Promise.all([attempt, nestedAttempt])).resolves.toEqual([undefined, undefined]);
  await expect(recoverRetryableCleanups()).resolves.toBeUndefined();
  expect(cleanup).toHaveBeenCalledOnce();
  expect(release).toHaveBeenCalledOnce();
});

test('external retaining caller keeps a shared non-retaining failure recoverable', async () => {
  const cleanupError = new Error('shared cleanup failure');
  let ownsResources = true;
  const cleanup = vi
    .fn<() => Promise<void>>()
    .mockRejectedValueOnce(cleanupError)
    .mockImplementationOnce(async () => {
      ownsResources = false;
    });
  trackRetryableCleanupOwnership(cleanup, () => ownsResources);

  const directAttempt = runRetryableCleanup(cleanup, false);
  const retainingAttempt = runRetryableCleanup(cleanup);

  await expect(Promise.allSettled([directAttempt, retainingAttempt])).resolves.toEqual([
    { status: 'rejected', reason: cleanupError },
    { status: 'rejected', reason: cleanupError },
  ]);
  expect(cleanup).toHaveBeenCalledOnce();

  await expect(recoverRetryableCleanups()).resolves.toBeUndefined();
  expect(cleanup).toHaveBeenCalledTimes(2);
});

test('external concurrent cleanup waits for the published attempt', async () => {
  let releaseCleanup!: () => void;
  const cleanupRelease = new Promise<void>((resolve) => {
    releaseCleanup = resolve;
  });
  const cleanup = vi.fn(() => cleanupRelease);

  const attempt = runRetryableCleanup(cleanup);
  const concurrentAttempt = runRetryableCleanup(cleanup);
  let concurrentSettled = false;
  void concurrentAttempt.then(() => {
    concurrentSettled = true;
  });

  await Promise.resolve();
  expect(cleanup).toHaveBeenCalledOnce();
  expect(concurrentSettled).toBe(false);

  releaseCleanup();
  await expect(Promise.all([attempt, concurrentAttempt])).resolves.toEqual([undefined, undefined]);
});
