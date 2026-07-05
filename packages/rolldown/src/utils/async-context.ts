import { AsyncLocalStorage } from 'node:async_hooks';

export interface AsyncContext<T> {
  getStore(): T | undefined;
  run<R>(store: T, callback: () => R): R;
}

export function createAsyncContext<T>(): AsyncContext<T> | undefined {
  return import.meta.browserBuild ? undefined : new AsyncLocalStorage<T>();
}
