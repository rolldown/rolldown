export type RetryableCleanup = () => Promise<void>;
type AbandonedRecoveryPolicy = 'setup' | false;

interface RetryableCleanupClaim {
  cleanup?: RetryableCleanup;
}

interface CleanupAttemptState {
  promise: Promise<void>;
  invokingCleanup: boolean;
  retainFailureRequested: boolean;
}

const retryableCleanupClaims = new WeakMap<object, RetryableCleanupClaim>();
const claimsByCleanup = new WeakMap<RetryableCleanup, Set<RetryableCleanupClaim>>();
const cleanupOwnershipChecks = new WeakMap<RetryableCleanup, () => boolean>();
const abandonedRecoveryPolicies = new WeakMap<RetryableCleanup, AbandonedRecoveryPolicy>();
const cleanupFailureErrors = new WeakSet<object>();
const pendingAbandonedCleanups = new Set<RetryableCleanup>();
const cleanupAttempts = new WeakMap<RetryableCleanup, CleanupAttemptState>();
const acknowledgedSelfReentry = Promise.resolve();
let pendingAbandonedCleanupRecovery: Promise<void> | undefined;

/** @internal Associate cleanup ownership with an error without changing its public shape. */
export function attachRetryableCleanup(error: Error, cleanup: RetryableCleanup): void {
  detachRetryableCleanupClaim(error);
  if (!hasRetryableCleanupOwnership(cleanup)) {
    clearRetryableCleanup(cleanup);
    return;
  }
  const claim: RetryableCleanupClaim = { cleanup };
  retryableCleanupClaims.set(error, claim);
  const claims = claimsByCleanup.get(cleanup);
  if (claims) {
    claims.add(claim);
  } else {
    claimsByCleanup.set(cleanup, new Set([claim]));
  }
  retainAbandonedCleanup(cleanup);
}

/** @internal Retrieve cleanup ownership retained by an error. */
export function getRetryableCleanup(error: unknown): RetryableCleanup | undefined {
  if (typeof error !== 'object' || error === null) return undefined;
  const cleanup = retryableCleanupClaims.get(error)?.cleanup;
  if (cleanup && !hasRetryableCleanupOwnership(cleanup)) {
    clearRetryableCleanup(cleanup);
    return undefined;
  }
  return cleanup;
}

/**
 * @internal Tell retry propagation whether a cleanup closure still owns resources.
 * Native close retries disable abandoned setup recovery because close hooks may
 * re-enter setup.
 * See internal-docs/async-runtime/implementation.md.
 */
export function trackRetryableCleanupOwnership(
  cleanup: RetryableCleanup,
  hasOwnership: () => boolean,
  { recoverAbandoned = 'setup' }: { recoverAbandoned?: AbandonedRecoveryPolicy } = {},
): void {
  cleanupOwnershipChecks.set(cleanup, hasOwnership);
  abandonedRecoveryPolicies.set(cleanup, recoverAbandoned);
  pendingAbandonedCleanups.delete(cleanup);
}

/** @internal Whether a cleanup closure still owns resources after its latest attempt. */
export function hasRetryableCleanupOwnership(cleanup: RetryableCleanup): boolean {
  return cleanupOwnershipChecks.get(cleanup)?.() ?? true;
}

function abandonedRecoveryPolicy(cleanup: RetryableCleanup): AbandonedRecoveryPolicy {
  return abandonedRecoveryPolicies.get(cleanup) ?? 'setup';
}

function retainAbandonedCleanup(cleanup: RetryableCleanup): void {
  if (abandonedRecoveryPolicy(cleanup) === 'setup') {
    pendingAbandonedCleanups.add(cleanup);
  }
}

/** @internal Identify an aggregate created while associating primary and cleanup failures. */
export function isCleanupFailureError(error: unknown): boolean {
  return typeof error === 'object' && error !== null && cleanupFailureErrors.has(error);
}

/** @internal Remove stale global ownership after a directly invoked cleanup succeeds. */
export function clearRetryableCleanup(cleanup: RetryableCleanup): void {
  pendingAbandonedCleanups.delete(cleanup);
  const claims = claimsByCleanup.get(cleanup);
  if (claims) {
    for (const claim of claims) {
      claim.cleanup = undefined;
    }
    claimsByCleanup.delete(cleanup);
  }
}

function detachRetryableCleanupClaim(error: object, expectedCleanup?: RetryableCleanup): boolean {
  const claim = retryableCleanupClaims.get(error);
  const cleanup = claim?.cleanup;
  if (!claim || !cleanup || (expectedCleanup && cleanup !== expectedCleanup)) {
    return false;
  }
  retryableCleanupClaims.delete(error);
  claim.cleanup = undefined;
  const claims = claimsByCleanup.get(cleanup);
  claims?.delete(claim);
  if (claims?.size === 0) {
    claimsByCleanup.delete(cleanup);
  }
  return true;
}

function runCleanupAttempt(
  cleanup: RetryableCleanup,
  retainFailure: boolean,
): { attempt: CleanupAttemptState; promise: Promise<void> } {
  const activeAttempt = cleanupAttempts.get(cleanup);
  if (activeAttempt) {
    // An on-stack cleanup cannot await its own attempt. Calls after cleanup()
    // returns still observe invokingCleanup=false and join the real promise.
    if (!activeAttempt.invokingCleanup && retainFailure) {
      activeAttempt.retainFailureRequested = true;
    }
    return {
      attempt: activeAttempt,
      promise: activeAttempt.invokingCleanup ? acknowledgedSelfReentry : activeAttempt.promise,
    };
  }

  let resolveAttempt!: () => void;
  let rejectAttempt!: (error: unknown) => void;
  const attempt = new Promise<void>((resolve, reject) => {
    resolveAttempt = resolve;
    rejectAttempt = reject;
  });
  const attemptState: CleanupAttemptState = {
    promise: attempt,
    invokingCleanup: true,
    retainFailureRequested: retainFailure,
  };
  cleanupAttempts.set(cleanup, attemptState);

  const finishAttempt = () => {
    if (cleanupAttempts.get(cleanup) === attemptState) {
      cleanupAttempts.delete(cleanup);
    }
  };
  let result: Promise<void>;
  try {
    result = cleanup();
  } catch (error) {
    attemptState.invokingCleanup = false;
    finishAttempt();
    rejectAttempt(error);
    return { attempt: attemptState, promise: attempt };
  }
  attemptState.invokingCleanup = false;
  void Promise.resolve(result).then(
    () => {
      finishAttempt();
      resolveAttempt();
    },
    (error: unknown) => {
      finishAttempt();
      rejectAttempt(error);
    },
  );
  return { attempt: attemptState, promise: attempt };
}

/** @internal Run one coalesced cleanup attempt and retain globally eligible failures. */
export async function runRetryableCleanup(
  cleanup: RetryableCleanup,
  retainFailure = true,
): Promise<void> {
  pendingAbandonedCleanups.delete(cleanup);
  const { attempt, promise } = runCleanupAttempt(cleanup, retainFailure);
  try {
    await promise;
  } catch (error) {
    if (hasRetryableCleanupOwnership(cleanup)) {
      if (attempt.retainFailureRequested) {
        retainAbandonedCleanup(cleanup);
      } else {
        pendingAbandonedCleanups.delete(cleanup);
      }
    } else {
      clearRetryableCleanup(cleanup);
    }
    throw error;
  }
  clearRetryableCleanup(cleanup);
}

/** @internal Wait until a later host event-loop turn before a final bounded retry. */
export function waitForRetryableCleanupTurn(): Promise<void> {
  return new Promise((resolve) => {
    globalThis.setTimeout(resolve, 0);
  });
}

/**
 * @internal Remove terminal diagnostics already surfaced by prior attempts,
 * preserving identity-based multiplicity.
 */
export function excludeDeliveredErrors(
  candidates: readonly unknown[],
  delivered: readonly unknown[],
): unknown[] {
  const deliveredCounts = new Map<unknown, number>();
  for (const error of delivered) {
    deliveredCounts.set(error, (deliveredCounts.get(error) ?? 0) + 1);
  }
  return candidates.filter((error) => {
    const remaining = deliveredCounts.get(error) ?? 0;
    if (remaining === 0) return true;
    if (remaining === 1) {
      deliveredCounts.delete(error);
    } else {
      deliveredCounts.set(error, remaining - 1);
    }
    return false;
  });
}

/** @internal Recover setup cleanups whose caller discarded the associated error. */
export function recoverRetryableCleanups(): Promise<void> {
  if (pendingAbandonedCleanupRecovery) return pendingAbandonedCleanupRecovery;

  let recovery!: Promise<void>;
  recovery = Promise.resolve().then(async () => {
    try {
      const errors: unknown[] = [];
      const attemptedCleanups = new Set<RetryableCleanup>();
      while (true) {
        let cleanup: RetryableCleanup | undefined;
        for (const candidate of pendingAbandonedCleanups) {
          if (!attemptedCleanups.has(candidate)) {
            cleanup = candidate;
            break;
          }
        }
        if (!cleanup) break;
        attemptedCleanups.add(cleanup);

        if (
          abandonedRecoveryPolicy(cleanup) !== 'setup' ||
          !hasRetryableCleanupOwnership(cleanup)
        ) {
          clearRetryableCleanup(cleanup);
          continue;
        }
        try {
          await runRetryableCleanup(cleanup);
        } catch (error) {
          errors.push(error);
        }
      }
      if (errors.length === 1) throw errors[0];
      if (errors.length > 1) {
        throw new AggregateError(errors, 'Pending parallel-plugin worker cleanup failed');
      }
    } finally {
      if (pendingAbandonedCleanupRecovery === recovery) {
        pendingAbandonedCleanupRecovery = undefined;
      }
    }
  });
  pendingAbandonedCleanupRecovery = recovery;
  return recovery;
}

/** @internal Retry cleanup retained by an earlier failure, then preserve that failure. */
export async function retryCleanupFromError(error: unknown, message: string): Promise<never> {
  const claim =
    typeof error === 'object' && error !== null ? retryableCleanupClaims.get(error) : undefined;
  const cleanup = claim?.cleanup;
  if (!cleanup) throw error;

  try {
    await runRetryableCleanup(cleanup);
  } catch (cleanupError) {
    throw createCleanupFailureErrorInternal(error, cleanupError, cleanup, message, claim);
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
  return createCleanupFailureErrorInternal(error, cleanupError, cleanup, message);
}

function createCleanupFailureErrorInternal(
  error: unknown,
  cleanupError: unknown,
  cleanup: RetryableCleanup | undefined,
  message: string,
  capturedClaim?: RetryableCleanupClaim,
): AggregateError {
  let errors = [error, cleanupError];
  let cause = error;
  if (
    cleanup &&
    error instanceof AggregateError &&
    isCleanupFailureError(error) &&
    (capturedClaim
      ? retryableCleanupClaims.get(error) === capturedClaim
      : getRetryableCleanup(error) === cleanup)
  ) {
    errors = [...error.errors, cleanupError];
    cause = error.cause;
    detachRetryableCleanupClaim(error, cleanup);
  }
  const aggregate = new AggregateError(errors, message, { cause });
  cleanupFailureErrors.add(aggregate);
  if (cleanup) {
    if (typeof cleanupError === 'object' && cleanupError !== null) {
      detachRetryableCleanupClaim(cleanupError, cleanup);
    }
    attachRetryableCleanup(aggregate, cleanup);
  }
  return aggregate;
}
