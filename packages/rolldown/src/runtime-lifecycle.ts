import * as binding from './binding.cjs';
import {
  getOrCreateWasiRuntimeLeaseManager,
  type RuntimeLease,
  WasiRuntimeLeaseManager,
} from './runtime-lease-manager';
import { getRuntimeCapabilitiesCompat } from './runtime-support';
import { BindingMismatchError } from './utils/binding-mismatch-error';

export type { RuntimeLease } from './runtime-lease-manager';

// Only legacy tokio-backed threaded-WASI artifacts hold their async runtime
// alive through explicit reference-counted leases (the compat shim
// synthesizes `backend: 'tokio'` for old bindings without a capability
// reporter). Every current binding runs the shared runtime, follows the
// automatic N-API environment lifecycle on every target, and uses no-op
// leases.
const capabilityBinding = binding as Record<PropertyKey, unknown>;
const loadedRuntimeCapabilities = getRuntimeCapabilitiesCompat();
const runtimeLeaseRequired =
  loadedRuntimeCapabilities.target === 'wasi-threads' &&
  loadedRuntimeCapabilities.backend === 'tokio';
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

  return {
    async acquire() {
      throw new BindingMismatchError(
        'The loaded threaded-WASI binding does not expose the acquireAsyncRuntime() lifecycle ' +
          'API. Reinstall Rolldown so the JavaScript package and binding versions match.',
      );
    },
  };
}

async function unavailableRuntimeLeaseAcquisition(): Promise<RuntimeLease> {
  throw new TypeError('Runtime lease acquisition is disabled for this binding');
}
