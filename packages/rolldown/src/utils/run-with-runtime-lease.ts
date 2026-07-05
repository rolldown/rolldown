import { getRuntimeCapabilities } from '../binding.cjs';
import { acquireRuntimeLease } from '../runtime-lifecycle';

// See internal-docs/async-runtime/implementation.md.
const requiresRuntimeLease = getRuntimeCapabilities().target === 'wasi-threads';

export function runWithRuntimeLease<T>(
  operation: () => Promise<T>,
  aggregateMessage: string,
): Promise<T> {
  if (!requiresRuntimeLease) {
    return operation();
  }

  const runtimeLease = acquireRuntimeLease();
  let result: Promise<T>;
  try {
    result = operation();
  } catch (error) {
    releaseAfterError(runtimeLease, error, aggregateMessage);
  }
  return result.then(
    (value) => {
      runtimeLease.release();
      return value;
    },
    (error) => {
      releaseAfterError(runtimeLease, error, aggregateMessage);
    },
  );
}

export function leaseAsyncFunction<Args extends unknown[], Result>(
  operation: (...args: Args) => Promise<Result>,
  aggregateMessage: string,
): (...args: Args) => Promise<Result> {
  if (!requiresRuntimeLease) {
    return operation;
  }
  return function (this: unknown, ...args: Args) {
    return runWithRuntimeLease(
      () => Reflect.apply(operation, this, args) as Promise<Result>,
      aggregateMessage,
    );
  };
}

export function runtimeLeaseRequired(): boolean {
  return requiresRuntimeLease;
}

function releaseAfterError(
  runtimeLease: ReturnType<typeof acquireRuntimeLease>,
  error: unknown,
  aggregateMessage: string,
): never {
  try {
    runtimeLease.release();
  } catch (cleanupError) {
    throw new AggregateError([error, cleanupError], aggregateMessage, { cause: error });
  }
  throw error;
}
