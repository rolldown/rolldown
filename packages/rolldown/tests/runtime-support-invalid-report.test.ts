// @ts-nocheck This focused unit test mocks incompatible generated binding surfaces.
import { beforeEach, expect, test, vi } from 'vitest';

const binding = vi.hoisted(() => ({
  capabilityGetter: undefined as unknown,
  capabilityGetterError: undefined as unknown,
  capabilityGetterThrows: false,
  loadedTarget: 'wasi-threads',
  loadedTargetError: undefined as unknown,
  loadedTargetThrows: false,
  report: {
    asyncRuntimeBuild: false,
    backend: 'tokio',
    blockOnJsThreadSafe: false,
    devSupported: true,
    flavor: 'MultiThread',
    target: 'wasi-threads',
    threads: true,
    timers: true,
    wasi: true,
    watchSupported: false,
  } as Record<string, unknown>,
}));

vi.mock('../src/binding.cjs', () => ({
  get __rolldownBindingTarget() {
    if (binding.loadedTargetThrows) throw binding.loadedTargetError;
    return binding.loadedTarget;
  },
  get getRuntimeCapabilities() {
    if (binding.capabilityGetterThrows) throw binding.capabilityGetterError;
    return binding.capabilityGetter;
  },
}));

// @ts-ignore This focused unit test intentionally reaches package source outside the test rootDir.
import { getRuntimeCapabilitiesCompat, getRuntimeSupport } from '../src/runtime-support';

beforeEach(() => {
  binding.capabilityGetter = () => binding.report;
  binding.capabilityGetterError = undefined;
  binding.capabilityGetterThrows = false;
  binding.loadedTarget = 'wasi-threads';
  binding.loadedTargetError = undefined;
  binding.loadedTargetThrows = false;
  binding.report = {
    asyncRuntimeBuild: false,
    backend: 'tokio',
    blockOnJsThreadSafe: false,
    devSupported: true,
    flavor: 'MultiThread',
    target: 'wasi-threads',
    threads: true,
    timers: true,
    wasi: true,
    watchSupported: false,
  };
});

test('a present capability reporter export must be callable', () => {
  binding.capabilityGetter = null;

  expect(() => getRuntimeCapabilitiesCompat()).toThrow(
    expect.objectContaining({
      code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
      message: expect.stringContaining('getRuntimeCapabilities must be a function'),
    }),
  );
});

test('a throwing capability reporter export getter is wrapped as a binding mismatch', () => {
  const cause = new Error('capability export getter failed');
  binding.capabilityGetterError = cause;
  binding.capabilityGetterThrows = true;

  expect(() => getRuntimeCapabilitiesCompat()).toThrow(
    expect.objectContaining({
      cause,
      code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
      message: expect.stringContaining('binding export getRuntimeCapabilities could not be read'),
    }),
  );
});

test('a throwing capability reporter is wrapped as a binding mismatch', () => {
  const cause = new Error('capability reporter failed');
  binding.capabilityGetter = () => {
    throw cause;
  };

  expect(() => getRuntimeCapabilitiesCompat()).toThrow(
    expect.objectContaining({
      cause,
      code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
      message: expect.stringContaining('getRuntimeCapabilities() threw while reporting'),
    }),
  );
});

test('a throwing generated loader target getter is wrapped as a binding mismatch', () => {
  const cause = new Error('loader target getter failed');
  binding.loadedTargetError = cause;
  binding.loadedTargetThrows = true;

  expect(() => getRuntimeCapabilitiesCompat()).toThrow(
    expect.objectContaining({
      cause,
      code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
      message: expect.stringContaining('binding export __rolldownBindingTarget could not be read'),
    }),
  );
});

test('capability reports must include every non-legacy field', () => {
  delete binding.report.wasi;

  expect(() => getRuntimeCapabilitiesCompat()).toThrow(
    expect.objectContaining({
      code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
      message: expect.stringContaining('wasi must be a boolean'),
    }),
  );
});

test('throwing capability field getters are wrapped as binding mismatches', () => {
  const cause = new Error('threads getter failed');
  Object.defineProperty(binding.report, 'threads', {
    configurable: true,
    get() {
      throw cause;
    },
  });

  expect(() => getRuntimeCapabilitiesCompat()).toThrow(
    expect.objectContaining({
      cause,
      code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
      message: expect.stringContaining('threads could not be read'),
    }),
  );
});

test('throwing optional capability field getters are wrapped as binding mismatches', () => {
  const cause = new Error('watchSupported getter failed');
  Object.defineProperty(binding.report, 'watchSupported', {
    configurable: true,
    get() {
      throw cause;
    },
  });

  expect(() => getRuntimeCapabilitiesCompat()).toThrow(
    expect.objectContaining({
      cause,
      code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
      message: expect.stringContaining('watchSupported could not be read'),
    }),
  );
});

test('an undefined reporter failure remains available as the mismatch cause', () => {
  binding.capabilityGetter = () => {
    throw undefined;
  };

  let error: unknown;
  try {
    getRuntimeCapabilitiesCompat();
  } catch (caught) {
    error = caught;
  }

  expect(error).toMatchObject({
    code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
    message: expect.stringContaining('getRuntimeCapabilities() threw while reporting'),
  });
  expect(Object.prototype.hasOwnProperty.call(error, 'cause')).toBe(true);
  expect((error as Error).cause).toBeUndefined();
});

test('capability reports cannot contradict generated loader target metadata', () => {
  Object.assign(binding.report, {
    target: 'native',
    wasi: false,
    watchSupported: true,
  });

  expect(() => getRuntimeCapabilitiesCompat()).toThrow(
    expect.objectContaining({
      code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
      message: expect.stringContaining('does not match the generated loader target'),
    }),
  );
});

test('thread and flavor contradictions fail closed', () => {
  binding.report.threads = false;

  expect(() => getRuntimeCapabilitiesCompat()).toThrow(
    expect.objectContaining({
      code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
      message: expect.stringContaining('threads does not agree'),
    }),
  );
});

test('explicit workflow capabilities may override scheduler and target defaults', () => {
  Object.assign(binding.report, {
    devSupported: false,
    watchSupported: true,
  });

  expect(getRuntimeCapabilitiesCompat()).toMatchObject({
    devSupported: false,
    threads: true,
    wasi: true,
    watchSupported: true,
  });
  expect(getRuntimeSupport()).toMatchObject({
    dev: false,
    watch: true,
  });
});

test('a CurrentThread integration may explicitly enable dev support', () => {
  binding.loadedTarget = 'native';
  Object.assign(binding.report, {
    devSupported: true,
    flavor: 'CurrentThread',
    target: 'native',
    threads: false,
    wasi: false,
    watchSupported: true,
  });

  expect(getRuntimeCapabilitiesCompat()).toMatchObject({
    devSupported: true,
    flavor: 'CurrentThread',
    threads: false,
  });
  expect(getRuntimeSupport().dev).toBe(true);
});

test('a native integration may explicitly disable watch support', () => {
  binding.loadedTarget = 'native';
  Object.assign(binding.report, {
    target: 'native',
    wasi: false,
    watchSupported: false,
  });

  expect(getRuntimeCapabilitiesCompat()).toMatchObject({
    target: 'native',
    wasi: false,
    watchSupported: false,
  });
  expect(getRuntimeSupport().watch).toBe(false);
});
