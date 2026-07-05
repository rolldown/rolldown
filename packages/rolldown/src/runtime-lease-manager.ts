export interface RuntimeLease {
  release(): void;
}

export interface RuntimeControl {
  enabled: boolean;
  start(this: void): void;
  shutdown(this: void): void;
}

/**
 * The threaded-WASI binding starts with one implicit runtime owner. The first
 * public object consumes that owner; later objects explicitly retain another
 * owner. Native and threadless artifacts use no-op leases.
 */
export class WasiRuntimeLeaseManager {
  #activeLeases = 0;
  #initialLeaseAvailable = true;
  #failedReleases = new Set<{ released: boolean }>();
  readonly #control: RuntimeControl;

  constructor(control: RuntimeControl) {
    this.#control = control;
  }

  get activeLeases(): number {
    return this.#activeLeases;
  }

  acquire(): RuntimeLease {
    if (!this.#control.enabled) {
      return NOOP_LEASE;
    }

    this.#recoverFailedReleases();

    if (this.#activeLeases > 0 || !this.#initialLeaseAvailable) {
      this.#control.start();
    } else {
      this.#initialLeaseAvailable = false;
    }
    this.#activeLeases += 1;

    const state = { released: false };
    return {
      release: () => {
        if (state.released) return;
        try {
          this.#control.shutdown();
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
        this.#control.shutdown();
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
      throw new AggregateError(errors, 'Failed to recover abandoned runtime lease releases');
    }
  }
}

const REGISTRY_KEY = Symbol.for('@rolldown/runtime-lease-managers/v1');

/**
 * Package copies that resolve the same binding must share the first-owner
 * state. Otherwise each copy could consume the binding's one implicit owner.
 */
export function getOrCreateWasiRuntimeLeaseManager(
  bindingIdentity: object,
  control: RuntimeControl,
  registryHost: object = globalThis,
): WasiRuntimeLeaseManager {
  if (!control.enabled) {
    return new WasiRuntimeLeaseManager(control);
  }

  let registry: WeakMap<object, WasiRuntimeLeaseManager>;
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
      throw new TypeError('Unable to safely establish the global Rolldown runtime lease registry');
    }
  } else {
    const existingRegistry = descriptor.value as unknown;
    try {
      if (descriptor.configurable || descriptor.enumerable || descriptor.writable) {
        throw new TypeError();
      }
      WeakMap.prototype.has.call(existingRegistry, bindingIdentity);
    } catch {
      throw new TypeError('The global Rolldown runtime lease registry is incompatible');
    }
    registry = existingRegistry as WeakMap<object, WasiRuntimeLeaseManager>;
  }

  let manager = WeakMap.prototype.get.call(registry, bindingIdentity) as
    | WasiRuntimeLeaseManager
    | undefined;
  if (!manager) {
    manager = new WasiRuntimeLeaseManager(control);
    WeakMap.prototype.set.call(registry, bindingIdentity, manager);
  }
  return manager;
}

const NOOP_LEASE: RuntimeLease = Object.freeze({
  release() {},
});
