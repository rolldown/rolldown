import { createAsyncContext } from './async-context';

interface CloseCallbackInvocation {
  active: boolean;
  browserCloseIdentityRetained: boolean;
  closeDependenciesByPromise: WeakMap<Promise<void>, Set<string>>;
  closeDependencyUnregisters: Set<() => void>;
  closeIdentity?: string;
  parent?: CloseCallbackInvocation;
  scope: CloseCallbackScope;
}

interface BrowserCloseDependencyCandidate {
  active: boolean;
  invocation: CloseCallbackInvocation;
  unregister: () => void;
}

const activeCloseCallback = createAsyncContext<CloseCallbackInvocation>();
const REENTRANT_CLOSE_ACKNOWLEDGEMENT = Promise.resolve();
const closeDependencies = new Map<string, Map<string, number>>();
const browserCloseIdentityCounts = new Map<string, number>();
let browserInvocation: CloseCallbackInvocation | undefined;
let nextCloseIdentity = 0n;

class CloseDependencyPromise extends Promise<void> {
  readonly #browserCandidates: BrowserCloseDependencyCandidate[] = [];
  readonly #closePromise: Promise<void>;
  readonly #selectObservedPromise: (
    closePromise: Promise<void>,
    browserInvocation?: CloseCallbackInvocation,
  ) => Promise<void>;

  static get [Symbol.species](): PromiseConstructor {
    return Promise;
  }

  constructor(
    closePromise: Promise<void>,
    selectObservedPromise: (
      closePromise: Promise<void>,
      browserInvocation?: CloseCallbackInvocation,
    ) => Promise<void>,
  ) {
    super((resolve, reject) => {
      void closePromise.then(resolve, reject);
    });
    this.#closePromise = closePromise;
    this.#selectObservedPromise = selectObservedPromise;
  }

  retainBrowserInvocation(invocation: CloseCallbackInvocation): void {
    const candidate = {
      active: true,
      invocation,
      unregister: () => {},
    };
    const unregister = () => {
      if (!candidate.active) return;
      candidate.active = false;
      invocation.closeDependencyUnregisters.delete(unregister);
    };
    candidate.unregister = unregister;
    this.#browserCandidates.push(candidate);
    invocation.closeDependencyUnregisters.add(unregister);
  }

  // oxlint-disable-next-line unicorn/no-thenable -- Promise observation is the dependency signal.
  override then<TResult1 = void, TResult2 = never>(
    onfulfilled?: ((value: void) => TResult1 | PromiseLike<TResult1>) | null,
    onrejected?: ((reason: any) => TResult2 | PromiseLike<TResult2>) | null,
  ): Promise<TResult1 | TResult2> {
    // Public observation is routed to the underlying close promise. Mark the
    // mirrored native state handled at the same time so observed failures do
    // not emit a duplicate rejection, while ignored closes still emit one.
    void super.then(undefined, () => {});
    return this.#selectObservedPromise(this.#closePromise, this.#takeBrowserInvocation()).then(
      onfulfilled,
      onrejected,
    );
  }

  #takeBrowserInvocation(): CloseCallbackInvocation | undefined {
    while (this.#browserCandidates.length > 0) {
      const candidate = this.#browserCandidates.pop()!;
      if (!candidate.active) continue;
      candidate.unregister();
      if (candidate.invocation.active) return candidate.invocation;
    }
  }
}

export function createCloseIdentity(namespace: string): string {
  nextCloseIdentity += 1n;
  return `${namespace}:${nextCloseIdentity}`;
}

/**
 * Marks callbacks that native close may be waiting on. A close requested from
 * one of those callbacks must let the callback return before the full close
 * lifecycle can settle.
 */
export class CloseCallbackScope {
  #defaultCloseIdentity: string | undefined;
  #dependencyAwarePromises = new WeakMap<Promise<void>, Map<string, CloseDependencyPromise>>();

  constructor(defaultCloseIdentity?: string) {
    this.#defaultCloseIdentity = defaultCloseIdentity;
  }

  run<T>(callback: () => T): T {
    return this.#run(
      this.#activeInvocation()?.closeIdentity ?? this.#defaultCloseIdentity,
      callback,
    );
  }

  runWithCloseIdentity<T>(closeIdentity: string, callback: () => T): T {
    return this.#run(closeIdentity, callback);
  }

  hasActiveCallback(): boolean {
    return this.#isActive(this.#defaultCloseIdentity);
  }

  #run<T>(closeIdentity: string | undefined, callback: () => T): T {
    const invocation: CloseCallbackInvocation = {
      active: true,
      browserCloseIdentityRetained: false,
      closeDependenciesByPromise: new WeakMap(),
      closeDependencyUnregisters: new Set(),
      closeIdentity,
      parent: this.#currentInvocation(),
      scope: this,
    };
    this.#retainBrowserCloseIdentity(invocation);
    return this.#runInvocation(invocation, () => this.#invoke(invocation, callback));
  }

  selectClosePromise(closePromise: Promise<void>, closeIdentity?: string): Promise<void> {
    if (this.#isActive(closeIdentity)) {
      return acknowledgeReentrantClose(closePromise);
    }

    const sourceInvocation = this.#activeInvocation();
    if (closeIdentity === undefined) {
      return closePromise;
    }

    if (
      sourceInvocation &&
      this.#wouldCompleteCloseDependencyCycle(closeIdentity, sourceInvocation)
    ) {
      return acknowledgeReentrantClose(closePromise);
    }

    const selectedPromise = this.#getDependencyAwarePromise(closePromise, closeIdentity);
    if (!activeCloseCallback && sourceInvocation) {
      selectedPromise.retainBrowserInvocation(sourceInvocation);
    }
    return selectedPromise;
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

  #isActive(closeIdentity: string | undefined): boolean {
    if (
      closeIdentity !== undefined &&
      !activeCloseCallback &&
      browserCloseIdentityCounts.has(closeIdentity)
    ) {
      return true;
    }

    if (closeIdentity === undefined) {
      let invocation = this.#currentInvocation();
      while (invocation) {
        if (invocation.active && invocation.scope === this) return true;
        invocation = invocation.parent;
      }
      return false;
    }

    return this.#activeInvocation(closeIdentity) !== undefined;
  }

  #activeInvocation(closeIdentity?: string): CloseCallbackInvocation | undefined {
    let invocation = this.#currentInvocation();
    while (invocation) {
      if (
        invocation.active &&
        (closeIdentity === undefined || invocation.closeIdentity === closeIdentity)
      ) {
        return invocation;
      }
      invocation = invocation.parent;
    }
  }

  #currentInvocation(): CloseCallbackInvocation | undefined {
    return activeCloseCallback?.getStore() ?? browserInvocation;
  }

  #hasCloseDependencyPath(sourceIdentity: string, targetIdentity: string): boolean {
    const pending = [sourceIdentity];
    const visited = new Set<string>();
    while (pending.length > 0) {
      const current = pending.pop()!;
      if (current === targetIdentity) return true;
      if (visited.has(current)) continue;
      visited.add(current);
      const dependencies = closeDependencies.get(current);
      if (dependencies) pending.push(...dependencies.keys());
    }
    return false;
  }

  #wouldCompleteCloseDependencyCycle(
    targetIdentity: string,
    sourceInvocation: CloseCallbackInvocation,
  ): boolean {
    return this.#activeIdentityInvocations(sourceInvocation).some(({ closeIdentity }) =>
      this.#hasCloseDependencyPath(targetIdentity, closeIdentity),
    );
  }

  #activeIdentityInvocations(
    sourceInvocation: CloseCallbackInvocation,
  ): Array<{ closeIdentity: string; invocation: CloseCallbackInvocation }> {
    const identities = new Set<string>();
    const invocations: Array<{
      closeIdentity: string;
      invocation: CloseCallbackInvocation;
    }> = [];
    let invocation: CloseCallbackInvocation | undefined = sourceInvocation;
    while (invocation) {
      const { closeIdentity } = invocation;
      if (invocation.active && closeIdentity !== undefined && !identities.has(closeIdentity)) {
        identities.add(closeIdentity);
        invocations.push({ closeIdentity, invocation });
      }
      invocation = invocation.parent;
    }
    return invocations;
  }

  #getDependencyAwarePromise(
    closePromise: Promise<void>,
    closeIdentity: string,
  ): CloseDependencyPromise {
    let promisesByIdentity = this.#dependencyAwarePromises.get(closePromise);
    if (!promisesByIdentity) {
      promisesByIdentity = new Map();
      this.#dependencyAwarePromises.set(closePromise, promisesByIdentity);
    }

    let selectedPromise = promisesByIdentity.get(closeIdentity);
    if (!selectedPromise) {
      selectedPromise = new CloseDependencyPromise(
        closePromise,
        (observedClosePromise, browserInvocation) =>
          this.#selectObservedClosePromise(observedClosePromise, closeIdentity, browserInvocation),
      );
      promisesByIdentity.set(closeIdentity, selectedPromise);
    }
    return selectedPromise;
  }

  #selectObservedClosePromise(
    closePromise: Promise<void>,
    closeIdentity: string,
    browserSourceInvocation?: CloseCallbackInvocation,
  ): Promise<void> {
    const sourceInvocation = this.#activeInvocation() ?? browserSourceInvocation;
    if (!sourceInvocation) return closePromise;

    const sourceInvocations = this.#activeIdentityInvocations(sourceInvocation);
    if (
      sourceInvocations.some(
        ({ closeIdentity: sourceIdentity }) => sourceIdentity === closeIdentity,
      )
    ) {
      return acknowledgeReentrantClose(closePromise);
    }
    if (
      sourceInvocations.some(({ closeIdentity: sourceIdentity }) =>
        this.#hasCloseDependencyPath(closeIdentity, sourceIdentity),
      )
    ) {
      return acknowledgeReentrantClose(closePromise);
    }

    for (const { closeIdentity: sourceIdentity, invocation } of sourceInvocations) {
      this.#registerCloseDependency(closePromise, invocation, sourceIdentity, closeIdentity);
    }
    return closePromise;
  }

  #registerCloseDependency(
    closePromise: Promise<void>,
    sourceInvocation: CloseCallbackInvocation,
    sourceIdentity: string,
    targetIdentity: string,
  ): void {
    let targets = sourceInvocation.closeDependenciesByPromise.get(closePromise);
    if (!targets) {
      targets = new Set();
      sourceInvocation.closeDependenciesByPromise.set(closePromise, targets);
    }
    if (targets.has(targetIdentity)) return;
    targets.add(targetIdentity);

    let dependencies = closeDependencies.get(sourceIdentity);
    if (!dependencies) {
      dependencies = new Map();
      closeDependencies.set(sourceIdentity, dependencies);
    }
    dependencies.set(targetIdentity, (dependencies.get(targetIdentity) ?? 0) + 1);

    const unregister = () => {
      if (!targets.delete(targetIdentity)) return;
      sourceInvocation.closeDependencyUnregisters.delete(unregister);

      const registeredDependencies = closeDependencies.get(sourceIdentity);
      const count = registeredDependencies?.get(targetIdentity);
      if (count === undefined) return;
      if (count <= 1) {
        registeredDependencies!.delete(targetIdentity);
        if (registeredDependencies!.size === 0) {
          closeDependencies.delete(sourceIdentity);
        }
      } else {
        registeredDependencies!.set(targetIdentity, count - 1);
      }
    };
    sourceInvocation.closeDependencyUnregisters.add(unregister);
    void closePromise.then(unregister, unregister);
  }

  #invoke<T>(invocation: CloseCallbackInvocation, callback: () => T): T {
    try {
      const result = callback();
      const then = getThen(result);
      if (!then) {
        this.#finishInvocation(invocation);
        return result;
      }
      return assimilateThenable(result, then, (callback) =>
        this.#runInvocation(invocation, callback),
      ).finally(() => {
        this.#finishInvocation(invocation);
      }) as T;
    } catch (error) {
      this.#finishInvocation(invocation);
      throw error;
    }
  }

  #retainBrowserCloseIdentity(invocation: CloseCallbackInvocation): void {
    if (activeCloseCallback || invocation.closeIdentity === undefined) return;

    const count = browserCloseIdentityCounts.get(invocation.closeIdentity) ?? 0;
    browserCloseIdentityCounts.set(invocation.closeIdentity, count + 1);
    invocation.browserCloseIdentityRetained = true;
  }

  #finishInvocation(invocation: CloseCallbackInvocation): void {
    if (!invocation.active) return;
    invocation.active = false;
    for (const unregister of invocation.closeDependencyUnregisters) unregister();
    if (!invocation.browserCloseIdentityRetained || invocation.closeIdentity === undefined) return;

    invocation.browserCloseIdentityRetained = false;
    const count = browserCloseIdentityCounts.get(invocation.closeIdentity);
    if (count === undefined || count <= 1) {
      browserCloseIdentityCounts.delete(invocation.closeIdentity);
    } else {
      browserCloseIdentityCounts.set(invocation.closeIdentity, count - 1);
    }
  }

  #runInvocation<T>(invocation: CloseCallbackInvocation, callback: () => T): T {
    if (activeCloseCallback) {
      return activeCloseCallback.run(invocation, callback);
    }

    // Browser hosts cannot propagate general async context. Keep the exact
    // synchronous invocation on the stack; identity-specific close privilege
    // is retained separately until the callback result settles.
    const previousInvocation = browserInvocation;
    browserInvocation = invocation;
    try {
      return callback();
    } finally {
      browserInvocation = previousInvocation;
    }
  }

  #wrapCallback(callback: Function, receiver?: object): Function {
    const run = <T>(invoke: () => T) => this.run(invoke);
    return function (this: unknown, ...args: unknown[]) {
      return run(() => Reflect.apply(callback, receiver ?? this, args));
    };
  }
}

function acknowledgeReentrantClose(closePromise: Promise<void>): Promise<void> {
  // The full result remains memoized for an external or later close call.
  // Attach a rejection handler because the reentrant caller cannot await the
  // result without recreating the callback/native-close cycle.
  void closePromise.catch(() => {});
  return REENTRANT_CLOSE_ACKNOWLEDGEMENT;
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

function assimilateThenable(
  value: unknown,
  then: Function,
  runSynchronousCallback: SynchronousCallbackRunner,
): Promise<unknown> {
  return new Promise((resolve, reject) => {
    settleThenable(
      value,
      then,
      new Set<object>([value as object]),
      runSynchronousCallback,
      resolve,
      reject,
    );
  });
}

function settleThenable(
  value: unknown,
  then: Function,
  thenableChain: Set<object>,
  runSynchronousCallback: SynchronousCallbackRunner,
  resolve: (value: unknown) => void,
  reject: (reason?: unknown) => void,
): void {
  // Match Promise.resolve's deferred then invocation. Box each resolution so
  // the native Promise algorithm cannot recursively assimilate a cyclic user
  // thenable before this resolver can inspect it.
  void Promise.resolve()
    .then(
      () =>
        new Promise<BoxedThenableResolution>((resolveThenable, rejectThenable) => {
          runSynchronousCallback(() => {
            Reflect.apply(then, value, [
              (resolved: unknown) =>
                resolveThenable(boxThenableResolution(thenableChain, resolved)),
              rejectThenable,
            ]);
          });
        }),
    )
    .then(
      (resolution) => resolveThenable(resolution, runSynchronousCallback, resolve, reject),
      reject,
    );
}

function resolveThenable(
  { thenableChain, value }: BoxedThenableResolution,
  runSynchronousCallback: SynchronousCallbackRunner,
  resolve: (value: unknown) => void,
  reject: (reason?: unknown) => void,
): void {
  if ((typeof value !== 'object' || value === null) && typeof value !== 'function') {
    resolve(value);
    return;
  }

  let then: unknown;
  try {
    runSynchronousCallback(() => {
      then = Reflect.get(value, 'then');
    });
  } catch (error) {
    reject(error);
    return;
  }
  if (typeof then !== 'function') {
    resolve(value);
    return;
  }
  if (thenableChain.has(value)) {
    reject(new TypeError('Thenable cycle detected while settling a callback result'));
    return;
  }

  const nextThenableChain = new Set(thenableChain);
  nextThenableChain.add(value);
  settleThenable(value, then, nextThenableChain, runSynchronousCallback, resolve, reject);
}

function boxThenableResolution(
  thenableChain: Set<object>,
  value: unknown,
): BoxedThenableResolution {
  const resolution = Object.create(null) as BoxedThenableResolution;
  resolution.thenableChain = thenableChain;
  resolution.value = value;
  return resolution;
}

type SynchronousCallbackRunner = <T>(callback: () => T) => T;

interface BoxedThenableResolution {
  thenableChain: Set<object>;
  value: unknown;
}
