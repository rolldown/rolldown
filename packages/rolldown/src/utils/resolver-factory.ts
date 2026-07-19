import { ResolverFactory as BindingResolverFactory, type ResolveResult } from '../binding.cjs';
import { runWithRuntimeLease, runtimeLeaseRequired } from './run-with-runtime-lease';

const PATCHED_RESOLVER_FACTORY = Symbol.for('@rolldown/resolver-factory-runtime-lease/v1');

type ResolverMethodName = 'async' | 'resolveFileAsync' | 'resolveDtsAsync';

if (runtimeLeaseRequired()) {
  const prototype = BindingResolverFactory.prototype as BindingResolverFactory & {
    [PATCHED_RESOLVER_FACTORY]?: boolean;
  };
  if (!prototype[PATCHED_RESOLVER_FACTORY]) {
    patchResolverMethod(prototype, 'async', 'Resolver and runtime release both failed');
    patchResolverMethod(
      prototype,
      'resolveFileAsync',
      'File resolution and runtime release both failed',
    );
    patchResolverMethod(
      prototype,
      'resolveDtsAsync',
      'Declaration resolution and runtime release both failed',
    );
    Object.defineProperty(prototype, PATCHED_RESOLVER_FACTORY, {
      configurable: false,
      enumerable: false,
      value: true,
      writable: false,
    });
  }
}

export const ResolverFactory: typeof BindingResolverFactory = BindingResolverFactory;

function patchResolverMethod(
  prototype: BindingResolverFactory,
  methodName: ResolverMethodName,
  aggregateMessage: string,
): void {
  const descriptor = Object.getOwnPropertyDescriptor(prototype, methodName);
  const original = descriptor?.value as
    | ((directory: string, request: string) => Promise<ResolveResult>)
    | undefined;
  if (!descriptor || !original) {
    throw new TypeError(`ResolverFactory.${methodName} is unavailable`);
  }
  Object.defineProperty(prototype, methodName, {
    ...descriptor,
    value(this: BindingResolverFactory, directory: string, request: string) {
      return runWithRuntimeLease(
        () => Reflect.apply(original, this, [directory, request]) as Promise<ResolveResult>,
        aggregateMessage,
      );
    },
  });
}
