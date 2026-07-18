import { BindingMismatchError } from './utils/binding-mismatch-error';

export interface RuntimeLease {
  release(): void;
}

export interface RuntimeControl {
  enabled: boolean;
  acquire(this: void): Promise<unknown>;
}

export interface LegacyRuntimeControl {
  enabled: boolean;
  shutdown(this: void): void;
  start(this: void): void;
}

/**
 * Every threaded-WASI operation owns one native runtime token. Native and
 * threadless artifacts use no-op leases.
 */
export class WasiRuntimeLeaseManager {
  #activeLeases = 0;
  #failedReleases = new Set<LeaseState>();
  readonly #control: RuntimeControl;

  constructor(control: RuntimeControl) {
    this.#control = control;
  }

  get activeLeases(): number {
    return this.#activeLeases;
  }

  acquire(): Promise<RuntimeLease> {
    if (!this.#control.enabled) {
      return Promise.resolve(NOOP_LEASE);
    }

    return this.#acquire();
  }

  async #acquire(): Promise<RuntimeLease> {
    this.#recoverFailedReleases();
    const nativeLease = validateNativeRuntimeLease(await this.#control.acquire());
    this.#activeLeases += 1;

    const state: LeaseState = { nativeLease, released: false };
    return {
      release: () => {
        if (state.released) return;
        try {
          releaseWithRetry(() => state.nativeLease.release());
        } catch (error) {
          this.#failedReleases.add(state);
          throw error;
        }
        this.#activeLeases -= 1;
        state.released = true;
        this.#failedReleases.delete(state);
      },
    };
  }

  #recoverFailedReleases(): void {
    const errors: unknown[] = [];
    for (const state of this.#failedReleases) {
      try {
        releaseWithRetry(() => state.nativeLease.release());
      } catch (error) {
        errors.push(error);
        continue;
      }
      this.#activeLeases -= 1;
      state.released = true;
      this.#failedReleases.delete(state);
    }
    if (errors.length === 1) throw errors[0];
    if (errors.length > 1) {
      throw new AggregateError(errors, 'Failed to recover abandoned runtime lease releases', {
        cause: errors[0],
      });
    }
  }
}

/**
 * Compatibility manager for bindings from before `acquireAsyncRuntime()`.
 * Those bindings start threaded WASI with one implicit owner, then expose
 * synchronous start/shutdown reference-count operations.
 */
export class LegacyWasiRuntimeLeaseManager {
  #activeLeases = 0;
  #failedReleases = new Set<LegacyLeaseState>();
  #initialLeaseAvailable = true;
  readonly #control: LegacyRuntimeControl;

  constructor(control: LegacyRuntimeControl) {
    this.#control = control;
  }

  get activeLeases(): number {
    return this.#activeLeases;
  }

  acquire(): Promise<RuntimeLease> {
    if (!this.#control.enabled) {
      return Promise.resolve(NOOP_LEASE);
    }

    return this.#acquire();
  }

  async #acquire(): Promise<RuntimeLease> {
    this.#recoverFailedReleases();
    if (this.#activeLeases > 0 || !this.#initialLeaseAvailable) {
      this.#control.start();
    } else {
      this.#initialLeaseAvailable = false;
    }
    this.#activeLeases += 1;

    const state: LegacyLeaseState = { released: false };
    return {
      release: () => {
        if (state.released) return;
        try {
          releaseWithRetry(this.#control.shutdown);
        } catch (error) {
          this.#failedReleases.add(state);
          throw error;
        }
        this.#activeLeases -= 1;
        state.released = true;
        this.#failedReleases.delete(state);
      },
    };
  }

  #recoverFailedReleases(): void {
    const errors: unknown[] = [];
    for (const state of this.#failedReleases) {
      try {
        releaseWithRetry(this.#control.shutdown);
      } catch (error) {
        errors.push(error);
        continue;
      }
      this.#activeLeases -= 1;
      state.released = true;
      this.#failedReleases.delete(state);
    }
    if (errors.length === 1) throw errors[0];
    if (errors.length > 1) {
      throw new AggregateError(
        errors,
        'Failed to recover abandoned legacy runtime lease releases',
        { cause: errors[0] },
      );
    }
  }
}

function releaseWithRetry(release: () => void): void {
  try {
    release();
  } catch {
    // A failed shutdown retains the same owner. Retry once before returning
    // control because setup and utility callers have no close object through
    // which another realm could recover a transient failure.
    release();
  }
}

function validateNativeRuntimeLease(value: unknown): RuntimeLease {
  if ((typeof value !== 'object' || value === null) && typeof value !== 'function') {
    throw new BindingRuntimeLeaseContractError();
  }
  let release: unknown;
  try {
    release = Reflect.get(value, 'release');
  } catch (error) {
    throw new BindingRuntimeLeaseContractError(error);
  }
  if (typeof release !== 'function') {
    throw new BindingRuntimeLeaseContractError();
  }
  return {
    release: () => Reflect.apply(release, value, []),
  };
}

class BindingRuntimeLeaseContractError extends BindingMismatchError {
  constructor(cause?: unknown) {
    super(
      'The loaded Rolldown binding returned an incompatible async runtime lease without a ' +
        'release() method. Reinstall Rolldown so the JavaScript package and binding versions match.',
      cause === undefined ? undefined : { cause },
    );
    this.name = 'BindingRuntimeContractError';
  }
}

const REGISTRY_KEY = Symbol.for('@rolldown/runtime-lease-managers/v1');

interface SharedRuntimeLeaseManager {
  acquire(): RuntimeLease | Promise<RuntimeLease>;
}

/**
 * Package copies that resolve the same binding share acquisition ordering and
 * failed-release recovery.
 */
export function getOrCreateWasiRuntimeLeaseManager(
  bindingIdentity: object,
  control: RuntimeControl,
  registryHost: object = globalThis,
): WasiRuntimeLeaseManager {
  if (!control.enabled) {
    return new WasiRuntimeLeaseManager(control);
  }

  return getOrCreateRuntimeLeaseManager(
    bindingIdentity,
    () => new WasiRuntimeLeaseManager(control),
    registryHost,
  );
}

/** Package-copy sharing for the legacy implicit-owner protocol. */
export function getOrCreateLegacyWasiRuntimeLeaseManager(
  bindingIdentity: object,
  control: LegacyRuntimeControl,
  registryHost: object = globalThis,
): LegacyWasiRuntimeLeaseManager {
  if (!control.enabled) {
    return new LegacyWasiRuntimeLeaseManager(control);
  }

  return getOrCreateRuntimeLeaseManager(
    bindingIdentity,
    () => new LegacyWasiRuntimeLeaseManager(control),
    registryHost,
    false,
  );
}

function getOrCreateRuntimeLeaseManager<T extends SharedRuntimeLeaseManager>(
  bindingIdentity: object,
  createManager: () => T,
  registryHost: object,
  allowLocalFallback = true,
): T {
  const registry = getWasiRuntimeLeaseRegistry(bindingIdentity, registryHost);
  if (!registry) {
    return createLocalManagerOrThrow(createManager, allowLocalFallback);
  }

  try {
    const manager = WeakMap.prototype.get.call(registry, bindingIdentity) as
      | SharedRuntimeLeaseManager
      | undefined;
    if (manager) {
      return typeof manager.acquire === 'function'
        ? (manager as T)
        : createLocalManagerOrThrow(createManager, allowLocalFallback);
    }
    const newManager = createManager();
    WeakMap.prototype.set.call(registry, bindingIdentity, newManager);
    return newManager;
  } catch {
    return createLocalManagerOrThrow(createManager, allowLocalFallback);
  }
}

function createLocalManagerOrThrow<T>(createManager: () => T, allowLocalFallback: boolean): T {
  if (allowLocalFallback) {
    return createManager();
  }
  throw new TypeError(
    'Unable to safely establish the global Rolldown runtime lease registry required by this legacy threaded-WASI binding',
  );
}

function getWasiRuntimeLeaseRegistry(
  bindingIdentity: object,
  registryHost: object,
): WeakMap<object, SharedRuntimeLeaseManager> | undefined {
  let registry: WeakMap<object, SharedRuntimeLeaseManager>;
  try {
    const descriptor = Object.getOwnPropertyDescriptor(registryHost, REGISTRY_KEY);
    if (descriptor === undefined) {
      registry = new WeakMap();
      if (
        !Reflect.defineProperty(registryHost, REGISTRY_KEY, {
          configurable: false,
          enumerable: false,
          value: registry,
          writable: false,
        })
      ) {
        return undefined;
      }
    } else {
      const existingRegistry = descriptor.value as unknown;
      if (descriptor.configurable || descriptor.enumerable || descriptor.writable) {
        return undefined;
      }
      WeakMap.prototype.has.call(existingRegistry, bindingIdentity);
      registry = existingRegistry as WeakMap<object, SharedRuntimeLeaseManager>;
    }
  } catch {
    return undefined;
  }
  return registry;
}

const NOOP_LEASE: RuntimeLease = Object.freeze({
  release() {},
});

interface LeaseState {
  nativeLease: RuntimeLease;
  released: boolean;
}

interface LegacyLeaseState {
  released: boolean;
}
