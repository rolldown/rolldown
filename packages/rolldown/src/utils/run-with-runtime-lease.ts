import { acquireRuntimeLease, isRuntimeLeaseRequired } from '../runtime-lifecycle';

// See internal-docs/async-runtime/implementation.md.
const requiresRuntimeLease = isRuntimeLeaseRequired();

export function runWithRuntimeLease<T>(
  operation: () => Promise<T>,
  aggregateMessage: string,
): Promise<T> {
  if (!requiresRuntimeLease) {
    return operation();
  }

  return runWithRequiredRuntimeLease(operation, aggregateMessage);
}

async function runWithRequiredRuntimeLease<T>(
  operation: () => Promise<T>,
  aggregateMessage: string,
): Promise<T> {
  const runtimeLease = await acquireRuntimeLease();
  let value: T;
  try {
    value = await operation();
  } catch (error) {
    releaseAfterError(runtimeLease, error, aggregateMessage);
  }
  runtimeLease.release();
  return value;
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
  runtimeLease: Awaited<ReturnType<typeof acquireRuntimeLease>>,
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
