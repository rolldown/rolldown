import { describe, expect, test, vi } from 'vitest';

const binding = vi.hoisted(() => ({
  acquireAsyncRuntime: vi.fn(),
  shutdownAsyncRuntime: undefined,
  startAsyncRuntime: undefined,
}));

vi.mock('../src/binding.cjs', () => binding);

// @ts-ignore These focused unit tests intentionally reach package source outside the test rootDir.
import { acquireRuntimeLease, CloseCoordinator } from '../src/runtime-lifecycle';
// @ts-ignore These focused unit tests intentionally reach package source outside the test rootDir.
import * as runtimeLease from '../src/runtime-lease-manager';

const {
  getOrCreateLegacyWasiRuntimeLeaseManager,
  getOrCreateWasiRuntimeLeaseManager,
  WasiRuntimeLeaseManager,
} = runtimeLease;

test('an incomplete binding mock uses the module-load native lease fallback', async () => {
  const lease = await acquireRuntimeLease();

  expect(() => lease.release()).not.toThrow();
  expect(binding.acquireAsyncRuntime).not.toHaveBeenCalled();
});

describe('WasiRuntimeLeaseManager', () => {
  test('acquires and releases one native token per active lease', async () => {
    const firstRelease = vi.fn();
    const secondRelease = vi.fn();
    const acquire = vi
      .fn()
      .mockResolvedValueOnce({ release: firstRelease })
      .mockResolvedValueOnce({ release: secondRelease });
    const manager = new WasiRuntimeLeaseManager({
      enabled: true,
      acquire,
    });

    const [first, second] = await Promise.all([manager.acquire(), manager.acquire()]);
    expect(acquire).toHaveBeenCalledTimes(2);
    expect(manager.activeLeases).toBe(2);

    first.release();
    first.release();
    expect(firstRelease).toHaveBeenCalledOnce();
    expect(manager.activeLeases).toBe(1);

    second.release();
    expect(secondRelease).toHaveBeenCalledOnce();
    expect(manager.activeLeases).toBe(0);
  });

  test('submits concurrent acquisitions immediately and recovers after rejection', async () => {
    const acquisitionError = new Error('acquisition failed');
    let rejectFirst!: (error: unknown) => void;
    const firstAcquisition = new Promise<{ release(): void }>((_, reject) => {
      rejectFirst = reject;
    });
    const release = vi.fn();
    const acquire = vi
      .fn()
      .mockReturnValueOnce(firstAcquisition)
      .mockResolvedValueOnce({ release });
    const manager = new WasiRuntimeLeaseManager({
      enabled: true,
      acquire,
    });

    const first = manager.acquire();
    const second = manager.acquire();
    expect(acquire).toHaveBeenCalledTimes(2);

    rejectFirst(acquisitionError);
    await expect(first).rejects.toBe(acquisitionError);
    const lease = await second;

    expect(acquire).toHaveBeenCalledTimes(2);
    expect(manager.activeLeases).toBe(1);
    lease.release();
    expect(release).toHaveBeenCalledOnce();
    expect(manager.activeLeases).toBe(0);
  });

  test('returns no-op leases outside threaded WASI', async () => {
    const acquire = vi.fn();
    const manager = new WasiRuntimeLeaseManager({
      enabled: false,
      acquire,
    });

    const lease = await manager.acquire();
    lease.release();
    expect(manager.activeLeases).toBe(0);
    expect(acquire).not.toHaveBeenCalled();
  });

  test('does not install a registry for disabled runtimes', async () => {
    const registryHost = Object.preventExtensions({});
    const manager = getOrCreateWasiRuntimeLeaseManager(
      function acquireAsyncRuntime() {},
      {
        enabled: false,
        acquire: vi.fn(),
      },
      registryHost,
    );

    const lease = await manager.acquire();
    expect(() => lease.release()).not.toThrow();
    expect(Reflect.ownKeys(registryHost)).toEqual([]);
  });

  test.each([
    ['a non-extensible registry host', Object.preventExtensions({})],
    [
      'an incompatible registry value',
      (() => {
        const host = {};
        Object.defineProperty(host, Symbol.for('@rolldown/runtime-lease-managers/v1'), {
          configurable: false,
          enumerable: false,
          value: {},
          writable: false,
        });
        return host;
      })(),
    ],
  ])('falls back to a local manager for %s', async (_name, registryHost) => {
    const release = vi.fn();
    const acquire = vi.fn().mockResolvedValue({ release });
    const manager = getOrCreateWasiRuntimeLeaseManager(
      function acquireAsyncRuntime() {},
      {
        enabled: true,
        acquire,
      },
      registryHost,
    );

    const lease = await manager.acquire();
    lease.release();
    expect(acquire).toHaveBeenCalledOnce();
    expect(release).toHaveBeenCalledOnce();
  });

  test.each([
    ['a non-extensible registry host', Object.preventExtensions({})],
    [
      'an incompatible registry value',
      (() => {
        const host = {};
        Object.defineProperty(host, Symbol.for('@rolldown/runtime-lease-managers/v1'), {
          configurable: false,
          enumerable: false,
          value: {},
          writable: false,
        });
        return host;
      })(),
    ],
  ])('fails closed for legacy implicit ownership with %s', (_name, registryHost) => {
    expect(() =>
      getOrCreateLegacyWasiRuntimeLeaseManager(
        function startAsyncRuntime() {},
        {
          enabled: true,
          shutdown: vi.fn(),
          start: vi.fn(),
        },
        registryHost,
      ),
    ).toThrow('global Rolldown runtime lease registry');
  });

  test('shares one lease manager across package copies', async () => {
    const acquire = vi
      .fn()
      .mockResolvedValueOnce({ release: vi.fn() })
      .mockResolvedValueOnce({ release: vi.fn() });
    const control = {
      enabled: true,
      acquire,
    };
    const bindingIdentity = function acquireAsyncRuntime() {};
    const registryHost = {};
    const firstModule = await import('../src/runtime-lease-manager');
    vi.resetModules();
    const secondModule = await import('../src/runtime-lease-manager');
    expect(firstModule.WasiRuntimeLeaseManager).not.toBe(secondModule.WasiRuntimeLeaseManager);

    const firstManager = firstModule.getOrCreateWasiRuntimeLeaseManager(
      bindingIdentity,
      control,
      registryHost,
    );
    const secondManager = secondModule.getOrCreateWasiRuntimeLeaseManager(
      bindingIdentity,
      control,
      registryHost,
    );

    const [first, second] = await Promise.all([firstManager.acquire(), secondManager.acquire()]);
    expect(firstManager).toBe(secondManager);
    expect(acquire).toHaveBeenCalledTimes(2);

    first.release();
    second.release();
    expect(secondManager.activeLeases).toBe(0);
  });

  test('uses independent native tokens when realms cannot share a registry', async () => {
    const releases = [vi.fn(), vi.fn()];
    const acquire = vi
      .fn()
      .mockResolvedValueOnce({ release: releases[0] })
      .mockResolvedValueOnce({ release: releases[1] });
    const bindingIdentity = function acquireAsyncRuntime() {};
    const control = {
      enabled: true,
      acquire,
    };
    const firstManager = getOrCreateWasiRuntimeLeaseManager(bindingIdentity, control, {});
    const secondManager = getOrCreateWasiRuntimeLeaseManager(bindingIdentity, control, {});

    expect(firstManager).not.toBe(secondManager);
    const [first, second] = await Promise.all([firstManager.acquire(), secondManager.acquire()]);
    expect(acquire).toHaveBeenCalledTimes(2);

    first.release();
    second.release();
    expect(releases[0]).toHaveBeenCalledOnce();
    expect(releases[1]).toHaveBeenCalledOnce();
  });

  test('does not retain a lease when native acquisition fails', async () => {
    const acquisitionError = new Error('acquisition failed');
    const release = vi.fn();
    const acquire = vi
      .fn()
      .mockRejectedValueOnce(acquisitionError)
      .mockResolvedValueOnce({ release });
    const manager = new WasiRuntimeLeaseManager({
      enabled: true,
      acquire,
    });

    await expect(manager.acquire()).rejects.toBe(acquisitionError);
    expect(manager.activeLeases).toBe(0);

    const lease = await manager.acquire();
    expect(manager.activeLeases).toBe(1);
    lease.release();
    expect(release).toHaveBeenCalledOnce();
    expect(manager.activeLeases).toBe(0);
  });

  test('retries a transient native release before another realm acquires', async () => {
    const release = vi
      .fn()
      .mockImplementationOnce(() => {
        throw new Error('release failed');
      })
      .mockImplementation(() => {});
    const acquire = vi.fn().mockResolvedValue({ release });
    const firstRealm = new WasiRuntimeLeaseManager({
      enabled: true,
      acquire,
    });
    const secondRealm = new WasiRuntimeLeaseManager({
      enabled: true,
      acquire,
    });

    const lease = await firstRealm.acquire();
    expect(() => lease.release()).not.toThrow();
    expect(firstRealm.activeLeases).toBe(0);
    expect(release).toHaveBeenCalledTimes(2);

    const next = await secondRealm.acquire();
    next.release();
    expect(acquire).toHaveBeenCalledTimes(2);
  });

  test('keeps a lease retryable when both native release attempts fail', async () => {
    const releaseError = new Error('release failed');
    const release = vi
      .fn()
      .mockImplementationOnce(() => {
        throw releaseError;
      })
      .mockImplementationOnce(() => {
        throw releaseError;
      })
      .mockImplementation(() => {});
    const acquire = vi.fn().mockResolvedValue({ release });
    const manager = new WasiRuntimeLeaseManager({
      enabled: true,
      acquire,
    });

    const lease = await manager.acquire();
    expect(() => lease.release()).toThrow(releaseError);
    expect(manager.activeLeases).toBe(1);
    expect(release).toHaveBeenCalledTimes(2);

    expect(() => lease.release()).not.toThrow();
    expect(manager.activeLeases).toBe(0);
    expect(release).toHaveBeenCalledTimes(3);

    lease.release();
    expect(release).toHaveBeenCalledTimes(3);
  });

  test('recovers every abandoned failed release before the next acquisition', async () => {
    const firstRelease = vi
      .fn()
      .mockImplementationOnce(() => {
        throw new Error('first release failed');
      })
      .mockImplementationOnce(() => {
        throw new Error('first release retry failed');
      })
      .mockImplementation(() => {});
    const secondRelease = vi
      .fn()
      .mockImplementationOnce(() => {
        throw new Error('second release failed');
      })
      .mockImplementationOnce(() => {
        throw new Error('second release retry failed');
      })
      .mockImplementation(() => {});
    const nextRelease = vi.fn();
    const acquire = vi
      .fn()
      .mockResolvedValueOnce({ release: firstRelease })
      .mockResolvedValueOnce({ release: secondRelease })
      .mockResolvedValueOnce({ release: nextRelease });
    const manager = new WasiRuntimeLeaseManager({
      enabled: true,
      acquire,
    });

    const [first, second] = await Promise.all([manager.acquire(), manager.acquire()]);
    expect(() => first.release()).toThrow('first release retry failed');
    expect(() => second.release()).toThrow('second release retry failed');
    expect(manager.activeLeases).toBe(2);

    const next = await manager.acquire();
    expect(firstRelease).toHaveBeenCalledTimes(3);
    expect(secondRelease).toHaveBeenCalledTimes(3);
    expect(acquire).toHaveBeenCalledTimes(3);
    expect(manager.activeLeases).toBe(1);

    first.release();
    second.release();
    expect(firstRelease).toHaveBeenCalledTimes(3);
    expect(secondRelease).toHaveBeenCalledTimes(3);

    next.release();
    expect(nextRelease).toHaveBeenCalledOnce();
    expect(manager.activeLeases).toBe(0);
  });
});

describe('CloseCoordinator', () => {
  test('coalesces an attempt and retries after a retryable cleanup failure', async () => {
    const cleanupError = new Error('cleanup failed');
    const attempt = vi
      .fn<() => Promise<{ errors: unknown[]; retryable: boolean }>>()
      .mockResolvedValueOnce({ errors: [cleanupError], retryable: true })
      .mockResolvedValue({ errors: [], retryable: false });
    const coordinator = new CloseCoordinator('close failed');

    const first = coordinator.close(attempt);
    const concurrent = coordinator.close(attempt);
    expect(concurrent).toBe(first);
    await expect(first).rejects.toBe(cleanupError);
    expect(attempt).toHaveBeenCalledOnce();

    await expect(coordinator.close(attempt)).resolves.toBeUndefined();
    expect(attempt).toHaveBeenCalledTimes(2);
  });

  test('replays terminal failures without rerunning completed phases', async () => {
    const terminalError = new Error('native close failed');
    const attempt = vi.fn(async () => ({
      errors: [terminalError],
      retryable: false,
    }));
    const coordinator = new CloseCoordinator('close failed');

    const first = coordinator.close(attempt);
    await expect(first).rejects.toBe(terminalError);
    const replay = coordinator.close(attempt);
    expect(replay).toBe(first);
    await expect(replay).rejects.toBe(terminalError);
    expect(attempt).toHaveBeenCalledOnce();
  });
});
