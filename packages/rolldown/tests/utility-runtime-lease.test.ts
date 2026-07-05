// @ts-nocheck This focused unit test mocks the generated binding surface.
import { expect, test, vi } from 'vitest';

const binding = vi.hoisted(() => {
  type Deferred = {
    reject(error: unknown): void;
    resolve(value: unknown): void;
  };

  const pending = new Map<string, Deferred[]>();
  const defer = (name: string) =>
    new Promise((resolve, reject) => {
      const entries = pending.get(name) ?? [];
      entries.push({ reject, resolve });
      pending.set(name, entries);
    });
  const settle = (name: string, action: (deferred: Deferred) => void) => {
    const deferred = pending.get(name)?.shift();
    if (!deferred) throw new Error(`No pending ${name} operation`);
    action(deferred);
  };

  class ResolverFactory {
    async() {
      return defer('resolve');
    }

    resolveFileAsync() {
      return defer('resolveFile');
    }

    resolveDtsAsync() {
      return defer('resolveDts');
    }
  }

  return {
    target: 'native',
    collapseSourcemaps: vi.fn(),
    driveCurrentThreadRuntimeTasks: vi.fn(),
    enhancedTransform: vi.fn(() => defer('transform')),
    enhancedTransformSync: vi.fn(),
    getRuntimeCapabilities: vi.fn(() => ({ target: binding.target })),
    isolatedDeclaration: vi.fn(() => defer('isolatedDeclaration')),
    isolatedDeclarationSync: vi.fn(),
    minify: vi.fn(() => defer('minify')),
    minifySync: vi.fn(),
    moduleRunnerTransform: vi.fn(() => defer('moduleRunnerTransform')),
    parse: vi.fn(() => defer('parse')),
    parseSync: vi.fn(),
    registerCurrentThreadTaskHost: vi.fn(),
    registerTimerHost: vi.fn(),
    reject: (name: string, error: unknown) => settle(name, (deferred) => deferred.reject(error)),
    resolve: (name: string, value: unknown) => settle(name, (deferred) => deferred.resolve(value)),
    ResolverFactory,
    shutdownAsyncRuntime: vi.fn(),
    startAsyncRuntime: vi.fn(),
  };
});

vi.mock('../src/binding.cjs', () => binding);
vi.mock('oxc-parser/src-js/wrap.js', () => ({
  wrap: (value: unknown) => value,
}));

test('public promise utilities lease only threaded-WASI runtime operations', async () => {
  binding.target = 'native';
  vi.resetModules();

  const nativeIsolatedDeclaration = await import('../src/utils/isolated-declaration');
  const nativeModuleRunnerTransform = await import('../src/utils/module-runner-transform');
  const nativeResolverFactory = await import('../src/utils/resolver-factory');

  expect(nativeIsolatedDeclaration.isolatedDeclaration).toBe(binding.isolatedDeclaration);
  expect(nativeModuleRunnerTransform.moduleRunnerTransform).toBe(binding.moduleRunnerTransform);
  expect(nativeResolverFactory.ResolverFactory).toBe(binding.ResolverFactory);
  expect(
    Object.getOwnPropertyDescriptor(nativeResolverFactory.ResolverFactory.prototype, 'async')
      ?.value,
  ).toBe(Object.getOwnPropertyDescriptor(binding.ResolverFactory.prototype, 'async')?.value);
  expect(binding.startAsyncRuntime).not.toHaveBeenCalled();
  expect(binding.shutdownAsyncRuntime).not.toHaveBeenCalled();

  binding.target = 'wasi-threads';
  vi.resetModules();

  const [
    { isolatedDeclaration },
    { moduleRunnerTransform },
    { ResolverFactory },
    { minify },
    { parse },
    { parseAstAsync },
    { transform },
  ] = await Promise.all([
    import('../src/utils/isolated-declaration'),
    import('../src/utils/module-runner-transform'),
    import('../src/utils/resolver-factory'),
    import('../src/utils/minify'),
    import('../src/utils/parse'),
    import('../src/parse-ast-index'),
    import('../src/utils/transform'),
  ]);

  const resolver = new ResolverFactory();
  const operations = [
    parse('input.js', 'export {}'),
    parseAstAsync('export {}'),
    transform('input.ts', 'const value: number = 1;'),
    minify('input.js', 'const value = 1;'),
    isolatedDeclaration('input.ts', 'export const value = 1;'),
    moduleRunnerTransform('input.js', 'export {}'),
    resolver.async('/project', './entry'),
    resolver.resolveFileAsync('/project/input.ts', './entry'),
    resolver.resolveDtsAsync('/project/input.d.ts', './entry'),
  ];

  // The first JS lease consumes the binding's implicit owner. Every
  // overlapping operation after it retains another native owner.
  expect(binding.startAsyncRuntime).toHaveBeenCalledTimes(operations.length - 1);
  expect(binding.shutdownAsyncRuntime).not.toHaveBeenCalled();

  const parseResult = {
    errors: [],
    program: { body: [], sourceType: 'module', type: 'Program' },
  };
  binding.resolve('parse', parseResult);
  binding.resolve('parse', parseResult);
  binding.resolve('transform', { code: 'const value = 1;\n', errors: [], warnings: [] });
  binding.resolve('minify', { code: 'const value=1;' });
  binding.resolve('isolatedDeclaration', { code: 'export declare const value = 1;', errors: [] });
  binding.resolve('moduleRunnerTransform', { code: 'export {}' });
  binding.resolve('resolve', { path: '/project/entry.js' });
  binding.resolve('resolveFile', { path: '/project/entry.js' });
  binding.resolve('resolveDts', { path: '/project/entry.d.ts' });

  await expect(Promise.all(operations)).resolves.toHaveLength(operations.length);
  expect(binding.shutdownAsyncRuntime).toHaveBeenCalledTimes(operations.length);

  const operationError = new Error('module transform failed');
  const rejectedOperation = moduleRunnerTransform('input.js', 'invalid');
  expect(binding.startAsyncRuntime).toHaveBeenCalledTimes(operations.length);
  binding.reject('moduleRunnerTransform', operationError);
  await expect(rejectedOperation).rejects.toBe(operationError);
  expect(binding.shutdownAsyncRuntime).toHaveBeenCalledTimes(operations.length + 1);

  // Once all owners stop the runtime, a later standalone utility restarts it.
  const restartedOperation = isolatedDeclaration('input.ts', 'export const restarted = true;');
  expect(binding.startAsyncRuntime).toHaveBeenCalledTimes(operations.length + 1);
  binding.resolve('isolatedDeclaration', { code: 'export declare const restarted = true;' });
  await expect(restartedOperation).resolves.toBeDefined();
  expect(binding.shutdownAsyncRuntime).toHaveBeenCalledTimes(operations.length + 2);

  const releaseError = new Error('runtime release failed');
  const primaryError = new Error('resolution failed');
  binding.shutdownAsyncRuntime.mockImplementationOnce(() => {
    throw releaseError;
  });
  const failedRelease = resolver.async('/project', './missing');
  binding.reject('resolve', primaryError);
  const aggregate = await failedRelease.catch((error: unknown) => error);
  expect(aggregate).toBeInstanceOf(AggregateError);
  expect(aggregate.errors).toEqual([primaryError, releaseError]);
  expect(aggregate.cause).toBe(primaryError);

  // The next acquisition first retries the retained failed release, then owns
  // and releases its new operation normally.
  const recoveredOperation = parse('input.js', 'export const recovered = true;');
  binding.resolve('parse', parseResult);
  await expect(recoveredOperation).resolves.toBe(parseResult);
});
