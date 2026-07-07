import { createAsyncContext } from './async-context';

interface CloseCallbackInvocation {
  active: boolean;
  scope: CloseCallbackScope;
}

const activeCloseCallback = createAsyncContext<CloseCallbackInvocation>();
const REENTRANT_CLOSE_ACKNOWLEDGEMENT = Promise.resolve();

/**
 * Marks callbacks that native close may be waiting on. A close requested from
 * one of those callbacks must let the callback return before the full close
 * lifecycle can settle.
 */
export class CloseCallbackScope {
  #browserInvocation: CloseCallbackInvocation | undefined;

  run<T>(callback: () => T): T {
    const invocation: CloseCallbackInvocation = {
      active: true,
      scope: this,
    };
    if (activeCloseCallback) {
      return activeCloseCallback.run(invocation, () => this.#invoke(invocation, callback));
    }

    // Browser hosts cannot propagate async context. Keep only the exact
    // synchronous invocation on the stack. See internal-docs/async-runtime/implementation.md.
    const previousInvocation = this.#browserInvocation;
    this.#browserInvocation = invocation;
    try {
      return this.#invoke(invocation, callback);
    } finally {
      this.#browserInvocation = previousInvocation;
    }
  }

  selectClosePromise(closePromise: Promise<void>): Promise<void> {
    if (!this.#isActive()) return closePromise;

    // The full result remains memoized for an external or later close call.
    // Attach a rejection handler because the reentrant caller cannot await the
    // result without recreating the callback/native-close cycle.
    void closePromise.catch(() => {});
    return REENTRANT_CLOSE_ACKNOWLEDGEMENT;
  }

  wrapCallbacks<T>(value: T): T {
    const clones = new WeakMap<object, object>();
    const visit = (current: unknown, wrapBuiltinConfig = false): unknown => {
      if (typeof current === 'function') {
        return this.#wrapCallback(current);
      }
      if (current === null || typeof current !== 'object') return current;

      const existing = clones.get(current);
      if (existing) return existing;

      if (Array.isArray(current)) {
        const clone: unknown[] = [];
        clone.length = current.length;
        clones.set(current, clone);
        for (const key of Reflect.ownKeys(current)) {
          if (key === 'length') continue;
          const descriptor = Object.getOwnPropertyDescriptor(current, key);
          if (!descriptor) continue;
          if ('value' in descriptor) descriptor.value = visit(descriptor.value);
          Object.defineProperty(clone, key, descriptor);
        }
        return clone;
      }

      const prototype = Object.getPrototypeOf(current);
      if (prototype !== Object.prototype && prototype !== null) {
        if (!wrapBuiltinConfig) return current;

        // Builtin options cross the binding as opaque objects, so inherited
        // class methods must be wrapped lazily without cloning the instance.
        const wrappedValues = new Map<PropertyKey, { source: unknown; wrapped: unknown }>();
        const clone = new Proxy(current, {
          get: (target, key) => {
            const source = Reflect.get(target, key, target);
            const cached = wrappedValues.get(key);
            if (cached && cached.source === source) return cached.wrapped;

            const wrapped =
              typeof source === 'function' ? this.#wrapCallback(source, target) : visit(source);
            wrappedValues.set(key, { source, wrapped });
            return wrapped;
          },
        });
        clones.set(current, clone);
        return clone;
      }

      const clone = Object.create(prototype) as Record<PropertyKey, unknown>;
      clones.set(current, clone);
      const isBuiltinPlugin = hasBuiltinPluginName(current);
      for (const key of Reflect.ownKeys(current)) {
        const descriptor = Object.getOwnPropertyDescriptor(current, key);
        if (!descriptor) continue;
        if ('value' in descriptor) {
          descriptor.value = visit(descriptor.value, isBuiltinPlugin && key === 'options');
        }
        Object.defineProperty(clone, key, descriptor);
      }
      return clone;
    };

    return visit(value) as T;
  }

  #isActive(): boolean {
    if (!activeCloseCallback) {
      return this.#browserInvocation?.active === true;
    }
    const invocation = activeCloseCallback.getStore();
    return invocation?.scope === this && invocation.active;
  }

  #invoke<T>(invocation: CloseCallbackInvocation, callback: () => T): T {
    try {
      const result = callback();
      const then = getThen(result);
      if (!then) {
        invocation.active = false;
        return result;
      }
      return assimilateThenable(result, then).finally(() => {
        invocation.active = false;
      }) as T;
    } catch (error) {
      invocation.active = false;
      throw error;
    }
  }

  #wrapCallback(callback: Function, receiver?: object): Function {
    const run = <T>(invoke: () => T) => this.run(invoke);
    return function (this: unknown, ...args: unknown[]) {
      return run(() => Reflect.apply(callback, receiver ?? this, args));
    };
  }
}

function hasBuiltinPluginName(value: object): boolean {
  const descriptor = Object.getOwnPropertyDescriptor(value, '__name');
  return typeof descriptor?.value === 'string' && descriptor.value.startsWith('builtin:');
}

function getThen(value: unknown): Function | undefined {
  if ((typeof value !== 'object' || value === null) && typeof value !== 'function') {
    return;
  }
  const then = Reflect.get(value, 'then');
  return typeof then === 'function' ? then : undefined;
}

function assimilateThenable(value: unknown, then: Function): Promise<unknown> {
  // Match Promise.resolve's deferred then invocation without reading a
  // user-controlled getter a second time. Box each resolution so the native
  // Promise algorithm cannot recursively assimilate a cyclic user thenable.
  const thenableChain = new Set<object>([value as object]);
  return invokeThenable(value, then, thenableChain).then(resolveThenable);
}

function invokeThenable(
  value: unknown,
  then: Function,
  thenableChain: Set<object>,
): Promise<BoxedThenableResolution> {
  return Promise.resolve().then(
    () =>
      new Promise((resolve, reject) => {
        Reflect.apply(then, value, [
          (resolved: unknown) => resolve({ thenableChain, value: resolved }),
          reject,
        ]);
      }),
  );
}

function resolveThenable({ thenableChain, value }: BoxedThenableResolution): unknown {
  if ((typeof value !== 'object' || value === null) && typeof value !== 'function') {
    return value;
  }
  if (thenableChain.has(value)) {
    throw new TypeError('Thenable cycle detected while settling a callback result');
  }

  const then = Reflect.get(value, 'then');
  if (typeof then !== 'function') return value;

  const nextThenableChain = new Set(thenableChain);
  nextThenableChain.add(value);
  return invokeThenable(value, then, nextThenableChain).then(resolveThenable);
}

interface BoxedThenableResolution {
  thenableChain: Set<object>;
  value: unknown;
}
