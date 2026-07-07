import * as binding from './binding.cjs';
import {
  getOrCreateWasiRuntimeLeaseManager,
  type RuntimeLease,
  WasiRuntimeLeaseManager,
} from './runtime-lease-manager';
import { getRuntimeCapabilitiesCompat } from './runtime-support';

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
    return (this.#closePromise ??= Promise.resolve().then(() => this.#run(attempt)));
  }

  async #run(attempt: () => Promise<CloseAttemptResult>): Promise<void> {
    const { errors, retryable } = await attempt();
    if (retryable) {
      this.#closePromise = undefined;
    }
    if (errors.length === 1) throw errors[0];
    if (errors.length > 1) {
      throw new AggregateError(errors, this.#aggregateMessage, { cause: errors[0] });
    }
  }
}

// See internal-docs/async-runtime/implementation.md.
const capabilityBinding = binding as Record<PropertyKey, unknown>;
const runtimeLeaseRequired = getRuntimeCapabilitiesCompat().target === 'wasi-threads';
const acquireAsyncRuntime =
  'acquireAsyncRuntime' in capabilityBinding ? capabilityBinding.acquireAsyncRuntime : undefined;

const runtimeLeaseManager = createRuntimeLeaseManager();

export async function acquireRuntimeLease(): Promise<RuntimeLease> {
  return runtimeLeaseManager.acquire();
}

/** @internal Snapshot taken once when this package copy loads. */
export function isRuntimeLeaseRequired(): boolean {
  return runtimeLeaseRequired;
}

function createRuntimeLeaseManager():
  | WasiRuntimeLeaseManager
  | { acquire(): Promise<RuntimeLease> } {
  if (!runtimeLeaseRequired) {
    return new WasiRuntimeLeaseManager({
      enabled: false,
      acquire: unavailableRuntimeLeaseAcquisition,
    });
  }

  if (typeof acquireAsyncRuntime === 'function') {
    const acquire = acquireAsyncRuntime as (this: void) => Promise<RuntimeLease>;
    return getOrCreateWasiRuntimeLeaseManager(acquire, {
      enabled: true,
      acquire,
    });
  }

  const startAsyncRuntime =
    'startAsyncRuntime' in capabilityBinding ? capabilityBinding.startAsyncRuntime : undefined;
  const shutdownAsyncRuntime =
    'shutdownAsyncRuntime' in capabilityBinding
      ? capabilityBinding.shutdownAsyncRuntime
      : undefined;
  if (typeof startAsyncRuntime === 'function' && typeof shutdownAsyncRuntime === 'function') {
    return {
      async acquire() {
        throw new TypeError(
          'The loaded threaded-WASI binding uses the legacy implicit runtime-owner protocol, ' +
            'which cannot be coordinated safely across JavaScript realms. Upgrade Rolldown to ' +
            'a binding that exposes acquireAsyncRuntime().',
        );
      },
    };
  }

  return {
    async acquire() {
      throw new TypeError(
        'The loaded threaded-WASI binding does not expose acquireAsyncRuntime() or the legacy ' +
          'startAsyncRuntime()/shutdownAsyncRuntime() lifecycle API. Reinstall Rolldown so the ' +
          'JavaScript package and binding versions match.',
      );
    },
  };
}

async function unavailableRuntimeLeaseAcquisition(): Promise<RuntimeLease> {
  throw new TypeError('Runtime lease acquisition is disabled for this binding');
}
