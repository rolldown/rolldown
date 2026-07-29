import { AsyncLocalStorage } from 'node:async_hooks';
import { findPropertyDescriptorInPrototypeChain } from './prototype-chain';

export interface AsyncContext<T> {
  getStore(): T | undefined;
  run<R>(store: T, callback: () => R): R;
}

/**
 * Storage for values that must remain visible through asynchronous continuations.
 *
 * `run()` must propagate `store` through promises and `await`, not only while
 * `callback` is executing synchronously. Rolldown validates the method shape but
 * cannot dynamically prove that propagation guarantee.
 */
export interface AsyncContextStorage {
  getStore(): unknown;
  run<R>(store: unknown, callback: () => R): R;
}

/**
 * Creates independent async-context storage.
 *
 * Each storage must provide host-backed asynchronous propagation equivalent to
 * Node.js `AsyncLocalStorage` or `AsyncContext.Variable`.
 */
export interface AsyncContextProvider {
  createStorage(): AsyncContextStorage;
}

/** The async-context provider currently selected by this Rolldown build. */
export interface AsyncContextSupport {
  source: 'custom' | 'native' | 'node' | 'unavailable';
  /** Whether the selected provider can create a storage object with the required methods. */
  supported: boolean;
}

export class AsyncContextUnavailableError extends Error {
  readonly code = 'ERR_ROLLDOWN_ASYNC_CONTEXT_UNAVAILABLE';

  constructor() {
    super(
      'JavaScript callbacks invoked by Rolldown in this browser require async-context propagation. ' +
        'Call configureAsyncContext() with a host-backed provider before building, ' +
        'or use a host that implements AsyncContext.Variable.',
    );
    this.name = 'AsyncContextUnavailableError';
  }
}

let configuredProvider: AsyncContextProvider | undefined;
let browserProviderLockDepth = 0;
let browserProviderAcquisition: BrowserProviderSelection | undefined;
let browserProviderSelection: BrowserProviderSelection | undefined;

/**
 * Configures the host-backed async context used by browser builds.
 *
 * The provider must propagate stores across promises and `await`. Configuration
 * remains replaceable until the first optional or required context selects a
 * provider.
 */
export function configureAsyncContext(provider: AsyncContextProvider): void {
  if (!import.meta.browserBuild) {
    throw new Error('Node.js builds use AsyncLocalStorage and cannot replace the async context');
  }
  if (browserProviderSelection || browserProviderLockDepth > 0) {
    throw new Error('Async context is already in use and can no longer be configured');
  }
  if (!provider) {
    throw new TypeError('Async context provider must define createStorage()');
  }
  browserProviderLockDepth += 1;
  try {
    // oxlint-disable-next-line typescript/unbound-method -- snapshotted with its receiver below
    const createStorage = provider.createStorage;
    if (typeof createStorage !== 'function') {
      throw new TypeError('Async context provider must define createStorage()');
    }
    if (browserProviderSelection) {
      throw new Error('Async context is already in use and can no longer be configured');
    }
    configuredProvider = {
      createStorage: () => Reflect.apply(createStorage, provider, []),
    };
  } finally {
    browserProviderLockDepth -= 1;
  }
}

/**
 * Reports whether the selected provider can create correctly shaped storage.
 *
 * This creates and discards a probe storage without locking configuration or
 * invoking its methods. It cannot verify asynchronous propagation semantics.
 */
export function getAsyncContextSupport(): AsyncContextSupport {
  if (!import.meta.browserBuild) {
    return { source: 'node', supported: true };
  }
  if (browserProviderSelection) {
    return {
      source: browserProviderSelection.source,
      supported: canCreateStorage(browserProviderSelection.provider),
    };
  }
  if (configuredProvider) {
    return { source: 'custom', supported: canCreateStorage(configuredProvider) };
  }
  const nativeProvider = getNativeAsyncContextProvider();
  if (nativeProvider) {
    return { source: 'native', supported: canCreateStorage(nativeProvider) };
  }
  return { source: 'unavailable', supported: false };
}

export function createAsyncContext<T>(): AsyncContext<T> | undefined {
  if (!import.meta.browserBuild) {
    return new AsyncLocalStorage<T>();
  }
  return createBrowserStorage<T>();
}

export function createRequiredAsyncContext<T>(): AsyncContext<T> {
  let storage: AsyncContext<T> | undefined;
  return {
    getStore() {
      return storage?.getStore();
    },
    run(storeValue, callback) {
      if (!storage) {
        if (import.meta.browserBuild) {
          storage = createRequiredBrowserStorage<T>();
        } else {
          storage = createStorage<T>(NODE_ASYNC_CONTEXT_PROVIDER);
        }
      }
      return storage.run(storeValue, callback);
    },
  };
}

export function trackAsyncCallbackSettlement<T>(
  result: T,
  onSettled: () => void,
  runSynchronousCallback: SynchronousCallbackRunner = runCallback,
): T {
  if (result === null || (typeof result !== 'object' && typeof result !== 'function')) {
    onSettled();
    return result;
  }

  const then = Reflect.get(result, 'then');
  if (typeof then !== 'function') {
    onSettled();
    return result;
  }

  let publicPromise: Promise<unknown> | undefined;
  const settlementPromise = assimilateThenable(
    result,
    then,
    runSynchronousCallback,
    () => publicPromise,
  );
  publicPromise = settlementPromise.finally(onSettled);
  return publicPromise as T;
}

const NODE_ASYNC_CONTEXT_PROVIDER: AsyncContextProvider = {
  createStorage: () => new AsyncLocalStorage<unknown>(),
};

function assimilateThenable(
  value: unknown,
  then: Function,
  runSynchronousCallback: SynchronousCallbackRunner,
  getPublicPromise: () => Promise<unknown> | undefined,
): Promise<unknown> {
  return new Promise((resolve, reject) => {
    settleThenable(
      value,
      then,
      new Set<object>([value as object]),
      runSynchronousCallback,
      getPublicPromise,
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
  getPublicPromise: () => Promise<unknown> | undefined,
  resolve: (value: unknown) => void,
  reject: (reason?: unknown) => void,
): void {
  // Match PromiseResolveThenableJob: invoke `then` later, but inspect a value
  // synchronously when the user resolving function receives it.
  let settled = false;
  void Promise.resolve().then(() => {
    const resolveOnce = (resolved: unknown) => {
      if (settled) return;
      settled = true;
      resolveThenable(
        resolved,
        thenableChain,
        runSynchronousCallback,
        getPublicPromise,
        resolve,
        reject,
      );
    };
    const rejectOnce = (reason?: unknown) => {
      if (settled) return;
      settled = true;
      reject(reason);
    };
    try {
      runSynchronousCallback(() => {
        // Promise-like resolution ignores the return value of `then`.
        Reflect.apply(then, value, [resolveOnce, rejectOnce]);
      });
    } catch (error) {
      rejectOnce(error);
    }
  });
}

function resolveThenable(
  value: unknown,
  thenableChain: Set<object>,
  runSynchronousCallback: SynchronousCallbackRunner,
  getPublicPromise: () => Promise<unknown> | undefined,
  resolve: (value: unknown) => void,
  reject: (reason?: unknown) => void,
): void {
  if ((typeof value !== 'object' || value === null) && typeof value !== 'function') {
    resolve(value);
    return;
  }

  if (value === getPublicPromise()) {
    reject(new TypeError('Thenable cycle detected while settling a callback result'));
    return;
  }

  if (thenableChain.has(value)) {
    let stillThenable: boolean;
    try {
      stillThenable = hasCallableThenWithoutInvokingAccessor(value);
    } catch (error) {
      reject(error);
      return;
    }
    if (stillThenable) {
      reject(new TypeError('Thenable cycle detected while settling a callback result'));
      return;
    }
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

  const nextThenableChain = new Set(thenableChain);
  nextThenableChain.add(value);
  settleThenable(
    value,
    then,
    nextThenableChain,
    runSynchronousCallback,
    getPublicPromise,
    resolve,
    reject,
  );
}

function hasCallableThenWithoutInvokingAccessor(value: object): boolean {
  const descriptor = findPropertyDescriptorInPrototypeChain(
    value,
    'then',
    'checking a repeated thenable resolution',
  );
  if (!descriptor) return false;
  if ('value' in descriptor) return typeof descriptor.value === 'function';

  // The same object already produced a callable `then` on this resolution
  // path. Treat an accessor that still exists as the same cycle without
  // invoking user code again. Deleting it still permits mutable self-resolution.
  return typeof descriptor.get === 'function';
}

function createBrowserStorage<T>(): AsyncContext<T> | undefined {
  if (browserProviderSelection) {
    return createStorage<T>(browserProviderSelection.provider);
  }

  browserProviderLockDepth += 1;
  let ownsAcquisition = false;
  let acquisition = browserProviderAcquisition;
  try {
    if (!acquisition) {
      const configured = configuredProvider;
      const provider = configured ?? getNativeAsyncContextProvider();

      // Native provider discovery can invoke accessors that reenter context
      // creation. A nested acquisition or selection has already escaped to its
      // caller and therefore wins over this frame's stale provider candidate.
      acquisition = browserProviderSelection ?? browserProviderAcquisition;
      if (!acquisition) {
        if (!provider) return undefined;
        acquisition = {
          provider,
          source: configured ? 'custom' : 'native',
        };
        browserProviderAcquisition = acquisition;
        ownsAcquisition = true;
      }
    }

    const storage = createStorage<T>(acquisition.provider);
    browserProviderSelection ??= acquisition;
    return storage;
  } finally {
    if (ownsAcquisition && browserProviderAcquisition === acquisition) {
      browserProviderAcquisition = undefined;
    }
    browserProviderLockDepth -= 1;
  }
}

function createRequiredBrowserStorage<T>(): AsyncContext<T> {
  const storage = createBrowserStorage<T>();
  if (!storage) throw new AsyncContextUnavailableError();
  return storage;
}

function createStorage<T>(provider: AsyncContextProvider): AsyncContext<T> {
  const storage = provider.createStorage();
  if (!isAsyncContextStorage(storage)) {
    throw new TypeError('Async context provider returned an invalid storage object');
  }
  return storage as AsyncContext<T>;
}

function canCreateStorage(provider: AsyncContextProvider): boolean {
  try {
    return isAsyncContextStorage(provider.createStorage());
  } catch {
    return false;
  }
}

function isAsyncContextStorage(storage: unknown): storage is AsyncContextStorage {
  return (
    storage != null &&
    typeof storage === 'object' &&
    typeof (storage as AsyncContextStorage).getStore === 'function' &&
    typeof (storage as AsyncContextStorage).run === 'function'
  );
}

function getNativeAsyncContextProvider(): AsyncContextProvider | undefined {
  const asyncContext = Reflect.get(globalThis, 'AsyncContext') as
    | {
        Variable?: new () => {
          get(): unknown;
          run<R>(store: unknown, callback: () => R): R;
        };
      }
    | undefined;
  const Variable = asyncContext?.Variable;
  if (typeof Variable !== 'function') return;
  return {
    createStorage() {
      const variable = new Variable();
      if (!variable || typeof variable.get !== 'function' || typeof variable.run !== 'function') {
        throw new TypeError('AsyncContext.Variable returned an invalid instance');
      }
      return {
        getStore: () => variable.get(),
        run: (store, callback) => variable.run(store, callback),
      };
    },
  };
}

function runCallback(callback: () => void): void {
  callback();
}

type SynchronousCallbackRunner = (callback: () => void) => void;

interface BrowserProviderSelection {
  provider: AsyncContextProvider;
  source: 'custom' | 'native';
}
