import { getRuntimeCapabilities, shutdownAsyncRuntime, startAsyncRuntime } from './binding.cjs';
import { getOrCreateWasiRuntimeLeaseManager, type RuntimeLease } from './runtime-lease-manager';

export type { RuntimeLease } from './runtime-lease-manager';

export interface CloseAttemptResult {
  errors: unknown[];
  retryable: boolean;
}

/**
 * Coalesce concurrent close calls and replay terminal results. Attempts with
 * retryable cleanup failures are cleared after settlement so a later close
 * can retry only the phases that retained ownership.
 */
export class CloseCoordinator {
  #closePromise: Promise<void> | undefined;
  readonly #aggregateMessage: string;

  constructor(aggregateMessage: string) {
    this.#aggregateMessage = aggregateMessage;
  }

  close(attempt: () => Promise<CloseAttemptResult>): Promise<void> {
    return (this.#closePromise ??= this.#run(attempt));
  }

  async #run(attempt: () => Promise<CloseAttemptResult>): Promise<void> {
    const { errors, retryable } = await attempt();
    if (retryable) {
      this.#closePromise = undefined;
    }
    if (errors.length === 1) throw errors[0];
    if (errors.length > 1) {
      throw new AggregateError(errors, this.#aggregateMessage);
    }
  }
}

// See internal-docs/async-runtime/implementation.md.
const runtimeLeaseManager = getOrCreateWasiRuntimeLeaseManager(startAsyncRuntime, {
  enabled: getRuntimeCapabilities().target === 'wasi-threads',
  start: startAsyncRuntime,
  shutdown: shutdownAsyncRuntime,
});

export function acquireRuntimeLease(): RuntimeLease {
  return runtimeLeaseManager.acquire();
}
