import { beforeEach, expect, test, vi } from 'vitest';

const binding = vi.hoisted(() => ({
  exportErrors: new Map<string, unknown>(),
  getRuntimeCapabilities: undefined as undefined | ReturnType<typeof vi.fn>,
  getCurrentThreadTaskHostContractVersion: undefined as undefined | (() => unknown),
  isCurrentThreadHostRegistrationActive: undefined as undefined | ReturnType<typeof vi.fn>,
  registerCurrentThreadTaskHost: undefined as undefined | ReturnType<typeof vi.fn>,
  registerTimerHost: undefined as undefined | ReturnType<typeof vi.fn>,
  reserveCurrentThreadHostRegistration: undefined as undefined | ReturnType<typeof vi.fn>,
  unregisterCurrentThreadTaskHost: undefined as undefined | (() => void),
  unregisterTimerHost: undefined as undefined | (() => void),
}));

vi.mock('../src/binding.cjs', () => ({
  get getRuntimeCapabilities() {
    if (binding.exportErrors.has('getRuntimeCapabilities')) {
      throw binding.exportErrors.get('getRuntimeCapabilities');
    }
    return binding.getRuntimeCapabilities;
  },
  get getCurrentThreadTaskHostContractVersion() {
    if (binding.exportErrors.has('getCurrentThreadTaskHostContractVersion')) {
      throw binding.exportErrors.get('getCurrentThreadTaskHostContractVersion');
    }
    return binding.getCurrentThreadTaskHostContractVersion;
  },
  get isCurrentThreadHostRegistrationActive() {
    return binding.isCurrentThreadHostRegistrationActive;
  },
  get registerCurrentThreadTaskHost() {
    return binding.registerCurrentThreadTaskHost;
  },
  get registerTimerHost() {
    return binding.registerTimerHost;
  },
  get reserveCurrentThreadHostRegistration() {
    return binding.reserveCurrentThreadHostRegistration;
  },
  get unregisterCurrentThreadTaskHost() {
    return binding.unregisterCurrentThreadTaskHost;
  },
  get unregisterTimerHost() {
    return binding.unregisterTimerHost;
  },
}));

beforeEach(() => {
  vi.resetModules();
  vi.clearAllMocks();
  binding.exportErrors.clear();
  binding.getRuntimeCapabilities = vi.fn(() => ({
    asyncRuntimeBuild: true,
    backend: 'shared',
    blockOnJsThreadSafe: false,
    devSupported: false,
    flavor: 'CurrentThread',
    target: 'native',
    threads: false,
    timers: false,
    wasi: false,
    watchSupported: true,
  }));
  binding.getCurrentThreadTaskHostContractVersion = undefined;
  binding.isCurrentThreadHostRegistrationActive = undefined;
  binding.registerCurrentThreadTaskHost = vi.fn((_dispatch?: unknown) => {});
  binding.registerTimerHost = vi.fn();
  binding.reserveCurrentThreadHostRegistration = undefined;
  binding.unregisterCurrentThreadTaskHost = undefined;
  binding.unregisterTimerHost = undefined;
});

test('rejects the previous callback-accepting binding before task-host invocation', async () => {
  await expect(import('../src/timer-host')).rejects.toMatchObject({
    code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
    message: expect.stringMatching(
      /incomplete async-runtime host contract.*getCurrentThreadTaskHostContractVersion/,
    ),
  });

  expect(binding.registerCurrentThreadTaskHost).not.toHaveBeenCalled();
  expect(binding.registerTimerHost).not.toHaveBeenCalled();
});

test('wraps a throwing capability export getter during host installation', async () => {
  const cause = new Error('capability getter failed');
  binding.exportErrors.set('getRuntimeCapabilities', cause);

  await expect(import('../src/timer-host')).rejects.toMatchObject({
    cause,
    code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
    message: expect.stringContaining('binding export getRuntimeCapabilities could not be read'),
  });
});

test('wraps a throwing async-runtime host export getter', async () => {
  const cause = new Error('contract getter failed');
  binding.exportErrors.set('getCurrentThreadTaskHostContractVersion', cause);

  await expect(import('../src/timer-host')).rejects.toMatchObject({
    cause,
    code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
    message: expect.stringContaining(
      'async-runtime host export getCurrentThreadTaskHostContractVersion could not be read',
    ),
  });
});

test('wraps a throwing async-runtime host contract reporter', async () => {
  const cause = 'contract reporter failed';
  binding.getCurrentThreadTaskHostContractVersion = vi.fn(() => {
    throw cause;
  });
  binding.isCurrentThreadHostRegistrationActive = vi.fn(() => true);
  binding.reserveCurrentThreadHostRegistration = vi.fn(() => ({ high: 0, low: 1 }));
  binding.unregisterCurrentThreadTaskHost = vi.fn();
  binding.unregisterTimerHost = vi.fn();

  await expect(import('../src/timer-host')).rejects.toMatchObject({
    cause,
    code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
    message: expect.stringContaining(
      'async-runtime host export getCurrentThreadTaskHostContractVersion threw while reporting',
    ),
  });
  expect(binding.registerCurrentThreadTaskHost).not.toHaveBeenCalled();
  expect(binding.registerTimerHost).not.toHaveBeenCalled();
});

test('rejects a nonnumeric host contract version without coercing it', async () => {
  const invalidVersion = {
    [Symbol.toPrimitive]() {
      throw new Error('contract version coercion must not run');
    },
  };
  binding.getCurrentThreadTaskHostContractVersion = vi.fn(() => invalidVersion);
  binding.isCurrentThreadHostRegistrationActive = vi.fn(() => true);
  binding.reserveCurrentThreadHostRegistration = vi.fn(() => ({ high: 0, low: 1 }));
  binding.unregisterCurrentThreadTaskHost = vi.fn();
  binding.unregisterTimerHost = vi.fn();

  await expect(import('../src/timer-host')).rejects.toMatchObject({
    code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
    message: expect.stringContaining('contract version a value of type object'),
  });
  expect(binding.registerCurrentThreadTaskHost).not.toHaveBeenCalled();
  expect(binding.registerTimerHost).not.toHaveBeenCalled();
});

test('rejects a reported shared runtime with no host contract', async () => {
  binding.registerCurrentThreadTaskHost = undefined;
  binding.registerTimerHost = undefined;
  binding.reserveCurrentThreadHostRegistration = undefined;

  await expect(import('../src/timer-host')).rejects.toMatchObject({
    code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
    message: expect.stringMatching(
      /incomplete async-runtime host contract.*registerCurrentThreadTaskHost/,
    ),
  });
});

test('allows a truly legacy binding with no capability reporter or host contract', async () => {
  binding.getRuntimeCapabilities = undefined;
  binding.registerCurrentThreadTaskHost = undefined;
  binding.registerTimerHost = undefined;

  await expect(import('../src/timer-host')).resolves.toBeDefined();
});

test('rejects an incomplete v4 reservation surface before registration', async () => {
  binding.getCurrentThreadTaskHostContractVersion = vi.fn(() => 4);
  binding.isCurrentThreadHostRegistrationActive = vi.fn(() => true);
  binding.unregisterCurrentThreadTaskHost = vi.fn();
  binding.unregisterTimerHost = vi.fn();

  await expect(import('../src/timer-host')).rejects.toMatchObject({
    code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
    message: expect.stringMatching(
      /incomplete async-runtime host contract.*reserveCurrentThreadHostRegistration/,
    ),
  });

  expect(binding.registerCurrentThreadTaskHost).not.toHaveBeenCalled();
  expect(binding.registerTimerHost).not.toHaveBeenCalled();
});

test('rejects the v3 native task-host contract before registration', async () => {
  binding.getCurrentThreadTaskHostContractVersion = vi.fn(() => 3);
  binding.isCurrentThreadHostRegistrationActive = vi.fn(() => true);
  binding.reserveCurrentThreadHostRegistration = vi.fn(() => ({ high: 0, low: 1 }));
  binding.unregisterCurrentThreadTaskHost = vi.fn();
  binding.unregisterTimerHost = vi.fn();

  await expect(import('../src/timer-host')).rejects.toMatchObject({
    code: 'ERR_ROLLDOWN_BINDING_MISMATCH',
    message: expect.stringMatching(/task-host contract version 3.*requires version 4/),
  });

  expect(binding.registerCurrentThreadTaskHost).not.toHaveBeenCalled();
  expect(binding.registerTimerHost).not.toHaveBeenCalled();
});
