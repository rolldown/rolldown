// @ts-nocheck This focused unit test mocks the generated binding surface.
import { expect, test, vi } from 'vitest';

const binding = vi.hoisted(() => {
  const registryConstructions: number[] = [];
  const workerSpawns: unknown[] = [];

  type Listener = (...args: unknown[]) => void;

  class FakeEmitter {
    listeners = new Map<string, Set<Listener>>();

    on(event: string, listener: Listener): this {
      const listeners = this.listeners.get(event) ?? new Set<Listener>();
      listeners.add(listener);
      this.listeners.set(event, listeners);
      return this;
    }

    off(event: string, listener: Listener): this {
      this.listeners.get(event)?.delete(listener);
      return this;
    }

    listenerCount(event: string): number {
      return this.listeners.get(event)?.size ?? 0;
    }

    emit(event: string, ...args: unknown[]): void {
      const listeners = this.listeners.get(event);
      if (!listeners) return;
      // Snapshot: a listener may unsubscribe while the event is dispatched.
      for (const listener of Array.from(listeners)) {
        listener(...args);
      }
    }
  }

  // A MessagePort pair that keeps the real port semantics the bootstrap
  // handshake depends on: delivery is asynchronous, and messages posted before
  // the peer subscribes stay buffered instead of being dropped.
  class FakeMessagePort extends FakeEmitter {
    peer: FakeMessagePort | undefined;
    closed = false;
    queue: unknown[] = [];

    postMessage(message: unknown): void {
      const peer = this.peer;
      if (!peer || peer.closed) return;
      peer.queue.push(message);
      queueMicrotask(() => peer.flush());
    }

    flush(): void {
      if (this.closed) {
        this.queue.length = 0;
        return;
      }
      if (this.listenerCount('message') === 0) return;
      while (this.queue.length > 0) {
        this.emit('message', this.queue.shift());
      }
    }

    override on(event: string, listener: Listener): this {
      super.on(event, listener);
      if (event === 'message') queueMicrotask(() => this.flush());
      return this;
    }

    close(): void {
      this.closed = true;
      this.queue.length = 0;
    }

    ref(): void {}
    unref(): void {}
    start(): void {}
  }

  class FakeMessageChannel {
    port1 = new FakeMessagePort();
    port2 = new FakeMessagePort();

    constructor() {
      this.port1.peer = this.port2;
      this.port2.peer = this.port1;
    }
  }

  // The supervisor authenticates every control message with tokens it embeds
  // in the generated bootstrap source, so the stand-in worker replies with the
  // very tokens the real worker would have read out of that source.
  const BOOTSTRAP_AUTHENTICATION = /const authentication = Object\.freeze\((\{[^{}]*\})\);/;

  class FakeWorker extends FakeEmitter {
    controlPort: FakeMessagePort;

    constructor(source: string, options: { workerData: { controlPort: FakeMessagePort } }) {
      super();
      workerSpawns.push([source, options]);
      const match = BOOTSTRAP_AUTHENTICATION.exec(source);
      if (!match) {
        throw new Error('the parallel-plugin bootstrap did not embed its authentication tokens');
      }
      const authentication = JSON.parse(match[1]) as Record<string, string>;
      const controlPort = options.workerData.controlPort;
      this.controlPort = controlPort;
      controlPort.on('message', (message: unknown) => {
        const start = message as Record<string, unknown> | null;
        if (
          start?.session === authentication.session &&
          start.token === authentication.startToken &&
          start.type === 'start'
        ) {
          controlPort.postMessage({
            session: authentication.session,
            token: authentication.resultToken,
            type: 'success',
          });
        }
      });
      controlPort.postMessage({
        session: authentication.session,
        token: authentication.readyToken,
        type: 'ready',
      });
    }

    postMessage(): void {}
    ref(): void {}
    unref(): void {}

    terminate(): Promise<number> {
      this.controlPort.close();
      return Promise.resolve(0);
    }
  }

  const nativeSharedCapabilities: Record<string, unknown> = {
    asyncRuntimeBuild: true,
    backend: 'shared',
    blockOnJsThreadSafe: false,
    devSupported: true,
    flavor: 'MultiThread',
    target: 'native',
    threads: true,
    timers: true,
    wasi: false,
    watchSupported: true,
  };
  // The Node threaded-WASI artifact: reports `parallelPlugins: false` through
  // the support matrix while `import.meta.browserBuild` stays false.
  const threadedWasiCapabilities: Record<string, unknown> = {
    asyncRuntimeBuild: true,
    backend: 'shared',
    blockOnJsThreadSafe: false,
    devSupported: false,
    flavor: 'CurrentThread',
    target: 'wasi-threads',
    threads: false,
    timers: true,
    wasi: true,
    watchSupported: false,
  };
  return {
    capabilities: threadedWasiCapabilities as Record<string, unknown>,
    FakeMessageChannel,
    FakeWorker,
    nativeSharedCapabilities,
    registryConstructions,
    threadedWasiCapabilities,
    workerSpawns,
  };
});

vi.mock('../src/binding.cjs', () => ({
  getRuntimeCapabilities: () => binding.capabilities,
  ParallelJsPluginRegistry: class {
    id = 1;
    constructor(workerCount: number) {
      binding.registryConstructions.push(workerCount);
    }
  },
}));

// Worker spawning is the side effect the runtime gate must precede; a real
// spawn would load the dist worker entry and the real binding. The stand-ins
// play the worker side of the authenticated control-port handshake so the pool
// still initializes and shuts down through the production code path.
vi.mock('node:worker_threads', async (importOriginal) => ({
  ...(await importOriginal()),
  MessageChannel: binding.FakeMessageChannel,
  Worker: binding.FakeWorker,
}));

// @ts-ignore These focused unit tests intentionally reach package source outside the test rootDir.
import { defineParallelPlugin } from '../src/plugin/parallel-plugin';
// @ts-ignore These focused unit tests intentionally reach package source outside the test rootDir.
import { getRuntimeSupport } from '../src/runtime-support';
// @ts-ignore These focused unit tests intentionally reach package source outside the test rootDir.
import { initializeParallelPlugins } from '../src/utils/initialize-parallel-plugins';

test('the support matrix reports parallel plugins unsupported on threaded WASI', () => {
  expect(getRuntimeSupport().parallelPlugins).toBe(false);
});

test('defineParallelPlugin fails on an unsupported artifact instead of returning a factory', () => {
  expect(() => defineParallelPlugin('file:///parallel-plugin.mjs')).toThrowError(
    expect.objectContaining({
      code: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
      feature: 'parallelPlugins',
    }),
  );
});

test('materialized parallel markers fail before any registry or worker side effect', async () => {
  const materializedMarker = {
    name: 'materialized-parallel',
    _parallel: { fileUrl: 'file:///parallel-plugin.mjs', options: undefined },
  };

  await expect(initializeParallelPlugins([materializedMarker])).rejects.toMatchObject({
    code: 'ERR_ROLLDOWN_UNSUPPORTED_RUNTIME_FEATURE',
    feature: 'parallelPlugins',
  });
  expect(binding.registryConstructions).toEqual([]);
  expect(binding.workerSpawns).toEqual([]);
});

test('the gate does not affect supported native artifacts', async () => {
  binding.capabilities = binding.nativeSharedCapabilities;
  try {
    const factory = defineParallelPlugin('/parallel-plugin.mjs');
    const marker = factory({ mode: 'fast' });
    expect(marker._parallel.options).toEqual({ mode: 'fast' });

    const result = await initializeParallelPlugins([marker]);
    expect(result?.registry.id).toBe(1);
    expect(binding.registryConstructions).toHaveLength(1);
    expect(binding.workerSpawns.length).toBeGreaterThan(0);
    await result?.stopWorkers();
  } finally {
    binding.capabilities = binding.threadedWasiCapabilities;
  }
});
