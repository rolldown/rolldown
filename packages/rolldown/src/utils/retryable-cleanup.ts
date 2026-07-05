export type RetryableCleanup = () => Promise<void>;

const retryableCleanups = new WeakMap<object, RetryableCleanup>();
const cleanupOwnershipChecks = new WeakMap<RetryableCleanup, () => boolean>();
const cleanupFailureErrors = new WeakSet<object>();
const pendingCleanups = new Set<RetryableCleanup>();
const cleanupAttempts = new WeakMap<RetryableCleanup, Promise<void>>();
let pendingCleanupRecovery: Promise<void> | undefined;

/** @internal Associate cleanup ownership with an error without changing its public shape. */
export function attachRetryableCleanup(error: Error, cleanup: RetryableCleanup): void {
  retryableCleanups.set(error, cleanup);
  pendingCleanups.add(cleanup);
}

/** @internal Retrieve cleanup ownership retained by a setup error. */
export function getRetryableCleanup(error: unknown): RetryableCleanup | undefined {
  return typeof error === 'object' && error !== null ? retryableCleanups.get(error) : undefined;
}

/** @internal Tell retry propagation whether a cleanup closure still owns resources. */
export function trackRetryableCleanupOwnership(
  cleanup: RetryableCleanup,
  hasOwnership: () => boolean,
): void {
  cleanupOwnershipChecks.set(cleanup, hasOwnership);
}

/** @internal Whether a cleanup closure still owns resources after its latest attempt. */
export function hasRetryableCleanupOwnership(cleanup: RetryableCleanup): boolean {
  return cleanupOwnershipChecks.get(cleanup)?.() ?? true;
}

/** @internal Identify an aggregate created while associating primary and cleanup failures. */
export function isCleanupFailureError(error: unknown): boolean {
  return typeof error === 'object' && error !== null && cleanupFailureErrors.has(error);
}

/** @internal Remove stale global ownership after a directly invoked cleanup succeeds. */
export function clearRetryableCleanup(cleanup: RetryableCleanup): void {
  pendingCleanups.delete(cleanup);
}

function runCleanupAttempt(cleanup: RetryableCleanup): Promise<void> {
  const activeAttempt = cleanupAttempts.get(cleanup);
  if (activeAttempt) return activeAttempt;

  const attempt = cleanup().finally(() => {
    if (cleanupAttempts.get(cleanup) === attempt) {
      cleanupAttempts.delete(cleanup);
    }
  });
  cleanupAttempts.set(cleanup, attempt);
  return attempt;
}

/** @internal Run one coalesced cleanup attempt while retaining failed ownership globally. */
export async function runRetryableCleanup(
  cleanup: RetryableCleanup,
  retainFailure = true,
): Promise<void> {
  pendingCleanups.delete(cleanup);
  try {
    await runCleanupAttempt(cleanup);
  } catch (error) {
    if (retainFailure && hasRetryableCleanupOwnership(cleanup)) {
      pendingCleanups.add(cleanup);
    }
    throw error;
  }
}

/** @internal Recover setup cleanups whose caller discarded the associated error. */
export function recoverRetryableCleanups(): Promise<void> {
  return (pendingCleanupRecovery ??= (async () => {
    const errors: unknown[] = [];
    const cleanups = Array.from(pendingCleanups);
    for (const cleanup of cleanups) {
      try {
        await runRetryableCleanup(cleanup);
      } catch (error) {
        if (!hasRetryableCleanupOwnership(cleanup)) {
          continue;
        }
        errors.push(error);
      }
    }
    if (errors.length === 1) throw errors[0];
    if (errors.length > 1) {
      throw new AggregateError(errors, 'Pending parallel-plugin worker cleanup failed');
    }
  })().finally(() => {
    pendingCleanupRecovery = undefined;
  }));
}

/** @internal Retry cleanup retained by an earlier setup failure, then rethrow that failure. */
export async function retryCleanupFromError(error: unknown, message: string): Promise<never> {
  const cleanup = getRetryableCleanup(error);
  if (!cleanup) throw error;

  try {
    await runRetryableCleanup(cleanup);
  } catch (cleanupError) {
    if (!hasRetryableCleanupOwnership(cleanup)) {
      if (typeof error === 'object' && error !== null) {
        retryableCleanups.delete(error);
      }
      throw error;
    }
    throw createCleanupFailureError(error, cleanupError, cleanup, message);
  }
  if (typeof error === 'object' && error !== null) {
    retryableCleanups.delete(error);
  }
  throw error;
}

/** @internal Run owned cleanup while preserving both the primary failure and retry ownership. */
export async function cleanupAfterError(
  error: unknown,
  cleanup: RetryableCleanup | undefined,
  message: string,
): Promise<never> {
  if (!cleanup) throw error;
  try {
    await runRetryableCleanup(cleanup);
  } catch (cleanupError) {
    throw createCleanupFailureError(error, cleanupError, cleanup, message);
  }
  throw error;
}

/** @internal Keep the primary error first while retaining only live cleanup ownership. */
export function createCleanupFailureError(
  error: unknown,
  cleanupError: unknown,
  cleanup: RetryableCleanup | undefined,
  message: string,
): AggregateError {
  let errors = [error, cleanupError];
  let cause = error;
  if (
    cleanup &&
    error instanceof AggregateError &&
    isCleanupFailureError(error) &&
    retryableCleanups.get(error) === cleanup
  ) {
    errors = [...error.errors, cleanupError];
    cause = error.cause;
    retryableCleanups.delete(error);
  }
  const aggregate = new AggregateError(errors, message, { cause });
  cleanupFailureErrors.add(aggregate);
  if (cleanup) {
    if (
      typeof cleanupError === 'object' &&
      cleanupError !== null &&
      retryableCleanups.get(cleanupError) === cleanup
    ) {
      retryableCleanups.delete(cleanupError);
    }
    attachRetryableCleanup(aggregate, cleanup);
  }
  return aggregate;
}
