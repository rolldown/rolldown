// @ts-ignore These focused unit tests intentionally reach package source outside the test rootDir.
import { CloseCoordinator } from '../src/runtime-lifecycle';
// @ts-ignore These focused unit tests intentionally reach package source outside the test rootDir.
import * as runtimeLease from '../src/runtime-lease-manager';
import { describe, expect, test, vi } from 'vitest';

const { getOrCreateWasiRuntimeLeaseManager, WasiRuntimeLeaseManager } = runtimeLease;

describe('WasiRuntimeLeaseManager', () => {
  test('claims and releases the binding implicit owner without starting it', () => {
    const start = vi.fn();
    const shutdown = vi.fn();
    const manager = new WasiRuntimeLeaseManager({
      enabled: true,
      start,
      shutdown,
    });

    const lease = manager.acquire();
    expect(start).not.toHaveBeenCalled();
    expect(manager.activeLeases).toBe(1);

    lease.release();
    expect(shutdown).toHaveBeenCalledOnce();
    expect(manager.activeLeases).toBe(0);
  });

  test('reference-counts concurrent explicit leases', () => {
    const start = vi.fn();
    const shutdown = vi.fn();
    const manager = new WasiRuntimeLeaseManager({
      enabled: true,
      start,
      shutdown,
    });

    const first = manager.acquire();
    const second = manager.acquire();
    expect(manager.activeLeases).toBe(2);
    expect(start).toHaveBeenCalledOnce();

    first.release();
    first.release();
    expect(manager.activeLeases).toBe(1);
    expect(shutdown).toHaveBeenCalledTimes(1);

    const third = manager.acquire();
    expect(manager.activeLeases).toBe(2);
    expect(start).toHaveBeenCalledTimes(2);

    second.release();
    third.release();
    expect(manager.activeLeases).toBe(0);
    expect(shutdown).toHaveBeenCalledTimes(3);

    const restarted = manager.acquire();
    expect(start).toHaveBeenCalledTimes(3);
    restarted.release();
    expect(shutdown).toHaveBeenCalledTimes(4);
  });

  test('returns no-op leases outside threaded WASI', () => {
    const start = vi.fn();
    const shutdown = vi.fn();
    const manager = new WasiRuntimeLeaseManager({
      enabled: false,
      start,
      shutdown,
    });

    manager.acquire().release();
    expect(manager.activeLeases).toBe(0);
    expect(start).not.toHaveBeenCalled();
    expect(shutdown).not.toHaveBeenCalled();
  });

  test('shares implicit-owner state across package copies using one binding', () => {
    const start = vi.fn();
    const shutdown = vi.fn();
    const control = {
      enabled: true,
      start,
      shutdown,
    };
    const bindingIdentity = function startAsyncRuntime() {};
    const registryHost = {};
    const firstManager = getOrCreateWasiRuntimeLeaseManager(bindingIdentity, control, registryHost);
    const secondManager = getOrCreateWasiRuntimeLeaseManager(
      bindingIdentity,
      control,
      registryHost,
    );

    const first = firstManager.acquire();
    const second = secondManager.acquire();
    expect(firstManager).toBe(secondManager);
    expect(start).toHaveBeenCalledOnce();

    first.release();
    second.release();
    expect(shutdown).toHaveBeenCalledTimes(2);
    expect(secondManager.activeLeases).toBe(0);
  });

  test('does not retain a lease when restarting the runtime fails', () => {
    const start = vi.fn();
    const shutdown = vi.fn();
    const manager = new WasiRuntimeLeaseManager({
      enabled: true,
      start,
      shutdown,
    });

    manager.acquire().release();
    start.mockImplementationOnce(() => {
      throw new Error('start failed');
    });

    expect(() => manager.acquire()).toThrow('start failed');
    expect(manager.activeLeases).toBe(0);

    manager.acquire().release();
    expect(start).toHaveBeenCalledTimes(2);
    expect(shutdown).toHaveBeenCalledTimes(2);
  });

  test('keeps a lease retryable when runtime shutdown fails', () => {
    const start = vi.fn();
    const shutdown = vi
      .fn()
      .mockImplementationOnce(() => {
        throw new Error('shutdown failed');
      })
      .mockImplementation(() => {});
    const manager = new WasiRuntimeLeaseManager({
      enabled: true,
      start,
      shutdown,
    });

    const lease = manager.acquire();
    expect(() => lease.release()).toThrow('shutdown failed');
    expect(manager.activeLeases).toBe(1);

    expect(() => lease.release()).not.toThrow();
    expect(manager.activeLeases).toBe(0);
    expect(shutdown).toHaveBeenCalledTimes(2);
  });

  test('recovers every abandoned failed release before the next acquisition', () => {
    const start = vi.fn();
    const shutdown = vi
      .fn()
      .mockImplementationOnce(() => {
        throw new Error('first shutdown failed');
      })
      .mockImplementationOnce(() => {
        throw new Error('second shutdown failed');
      })
      .mockImplementation(() => {});
    const manager = new WasiRuntimeLeaseManager({
      enabled: true,
      start,
      shutdown,
    });

    const first = manager.acquire();
    const second = manager.acquire();
    expect(() => first.release()).toThrow('first shutdown failed');
    expect(() => second.release()).toThrow('second shutdown failed');
    expect(manager.activeLeases).toBe(2);

    const next = manager.acquire();
    expect(shutdown).toHaveBeenCalledTimes(4);
    expect(start).toHaveBeenCalledTimes(2);
    expect(manager.activeLeases).toBe(1);

    first.release();
    second.release();
    expect(shutdown).toHaveBeenCalledTimes(4);

    next.release();
    expect(shutdown).toHaveBeenCalledTimes(5);
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
