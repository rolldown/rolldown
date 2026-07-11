import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { getRuntimeCapabilities } from 'rolldown/experimental';
import { describe, expect, test } from 'vitest';

const testsDir = fileURLToPath(new URL('.', import.meta.url));
const caps = getRuntimeCapabilities();
const sharedCurrentThreadNative = caps.asyncRuntimeBuild && caps.target === 'native';

function runCurrentThreadChild(script: string) {
  return spawnSync(process.execPath, ['--input-type=module', '-e', script], {
    cwd: testsDir,
    encoding: 'utf8',
    env: {
      ...process.env,
      ROLLDOWN_PARK_DEADLINE_MS: '60000',
      ROLLDOWN_RUNTIME: 'single',
    },
    timeout: 45_000,
  });
}

const childPrelude = `
  import fs from 'node:fs';
  import os from 'node:os';
  import path from 'node:path';
  import { createRequire } from 'node:module';
  import { pathToFileURL } from 'node:url';
  import { setTimeout as delay } from 'node:timers/promises';

  const { watch } = await import('rolldown');
  const { getRuntimeCapabilities } = await import('rolldown/experimental');
  const capabilities = getRuntimeCapabilities();
  if (capabilities.flavor !== 'CurrentThread') {
    throw new Error('expected CurrentThread, received ' + capabilities.flavor);
  }

  const packageDirectory = path.dirname(
    createRequire(import.meta.url).resolve('rolldown/package.json'),
  );
  const sharedDirectory = path.join(packageDirectory, 'dist', 'shared');
  const timerHostChunk = fs.readdirSync(sharedDirectory).find((name) => {
    if (!name.endsWith('.mjs')) return false;
    const source = fs.readFileSync(path.join(sharedDirectory, name), 'utf8');
    return source.includes('registerCurrentThreadTaskHost') && source.includes('registerTimerHost');
  });
  if (!timerHostChunk) {
    throw new Error('timer-host chunk not found');
  }
  const timerHostUrl = pathToFileURL(path.join(sharedDirectory, timerHostChunk)).href;

  async function loadBinding() {
    const chunk = await import(timerHostUrl);
    for (const value of Object.values(chunk)) {
      if (typeof value !== 'function' || value.length !== 0) continue;
      try {
        const candidate = value();
        if (candidate && typeof candidate.then === 'function') {
          candidate.catch(() => {});
          continue;
        }
        if (candidate && typeof candidate.registerTimerHost === 'function') {
          return candidate;
        }
      } catch {}
    }
    throw new Error('binding factory not found');
  }

  function waitForEnd(watcher, setTimeoutHost) {
    return new Promise((resolve, reject) => {
      const timeout = setTimeoutHost(() => {
        watcher.off('event', onEvent);
        reject(new Error('watch END timed out'));
      }, 15000);
      const onEvent = (event) => {
        if (event.code === 'ERROR') {
          clearTimeout(timeout);
          watcher.off('event', onEvent);
          reject(event.error);
        } else if (event.code === 'END') {
          clearTimeout(timeout);
          watcher.off('event', onEvent);
          resolve();
        }
      };
      watcher.on('event', onEvent);
    });
  }
`;

describe.runIf(sharedCurrentThreadNative)('async-runtime JavaScript host lifecycle', () => {
  test(
    'cache-busted timer-host chunks replace a native-evicted marker exactly once',
    { timeout: 50_000 },
    () => {
      const child = runCurrentThreadChild(`
        ${childPrelude}
        const binding = await loadBinding();
        const installations = Reflect.get(
          globalThis,
          Symbol.for('rolldown.current-thread-host-installations.v4'),
          globalThis,
        );
        const initialInstallation = installations.get(binding.registerCurrentThreadTaskHost);
        const retiredRegistration = initialInstallation?.timerHostRegistration;
        if (!Array.isArray(retiredRegistration)) {
          throw new Error('installed timer-host registration marker not found');
        }
        binding.unregisterTimerHost(...retiredRegistration);
        if (binding.isCurrentThreadHostRegistrationActive(...retiredRegistration)) {
          throw new Error('retired timer-host registration remained active');
        }

        await import(timerHostUrl + '?host-replacement=one');
        const replacementRegistration = installations.get(
          binding.registerCurrentThreadTaskHost,
        )?.timerHostRegistration;
        if (
          !Array.isArray(replacementRegistration) ||
          replacementRegistration[0] === retiredRegistration[0] &&
            replacementRegistration[1] === retiredRegistration[1] ||
          !binding.isCurrentThreadHostRegistrationActive(...replacementRegistration)
        ) {
          throw new Error('cache-busted evaluation did not install one live replacement');
        }
        await import(timerHostUrl + '?host-replacement=two');
        const stableRegistration = installations.get(
          binding.registerCurrentThreadTaskHost,
        )?.timerHostRegistration;
        if (
          stableRegistration?.[0] !== replacementRegistration[0] ||
          stableRegistration?.[1] !== replacementRegistration[1]
        ) {
          throw new Error('a live replacement registration was duplicated');
        }

        const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'rd-host-dedupe-'));
        const input = path.join(directory, 'main.js');
        fs.writeFileSync(input, 'export const value = 1;');
        const watcher = watch({
          input,
          cwd: directory,
          output: { dir: path.join(directory, 'dist') },
          watch: {
            buildDelay: 175,
            watcher: { usePolling: true, pollInterval: 25 },
          },
        });
        const originalSetTimeout = globalThis.setTimeout;
        try {
          await waitForEnd(watcher, originalSetTimeout);
          let hostTimerArms = 0;
          globalThis.setTimeout = function (callback, timeout, ...args) {
            if (typeof timeout === 'number' && timeout >= 100 && timeout <= 400) {
              hostTimerArms += 1;
            }
            return Reflect.apply(originalSetTimeout, this, [callback, timeout, ...args]);
          };
          await delay(1100);
          const secondEnd = waitForEnd(watcher, originalSetTimeout);
          fs.writeFileSync(input, 'export const value = 2;');
          await secondEnd;
          console.log(JSON.stringify({ hostTimerArms }));
        } finally {
          globalThis.setTimeout = originalSetTimeout;
          await watcher.close();
          fs.rmSync(directory, { force: true, recursive: true });
        }
      `);

      expect(child.error, child.stderr).toBeUndefined();
      expect(child.status, child.stderr).toBe(0);
      const lines = child.stdout.trim().split('\n');
      expect(JSON.parse(lines[lines.length - 1])).toEqual({ hostTimerArms: 1 });
    },
  );

  test(
    'schedule rejection and cancellation failure share one strike per timer relay',
    { timeout: 50_000 },
    () => {
      const child = runCurrentThreadChild(`
        ${childPrelude}
        const binding = await loadBinding();
        let cancellationCount = 0;
        let resolveCancellations;
        const scheduleRejectors = new Map();
        const cancellationsObserved = new Promise((resolve) => {
          resolveCancellations = resolve;
        });
        const registration = binding.reserveCurrentThreadHostRegistration();
        binding.registerTimerHost(
          registration.high,
          registration.low,
          (id) => new Promise((_, reject) => {
            scheduleRejectors.set(id, reject);
          }),
          (id) => {
            cancellationCount += 1;
            const error = new Error('intentional timer relay failure');
            scheduleRejectors.get(id)?.(error);
            scheduleRejectors.delete(id);
            if (cancellationCount >= 3) resolveCancellations();
            throw error;
          },
        );

        const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'rd-host-cancel-'));
        const input = path.join(directory, 'main.js');
        fs.writeFileSync(input, 'export const value = 1;');
        const watcher = watch({
          input,
          cwd: directory,
          output: { dir: path.join(directory, 'dist') },
          watch: {
            buildDelay: 175,
            watcher: { usePolling: true, pollInterval: 25 },
          },
        });
        const originalSetTimeout = globalThis.setTimeout;
        try {
          await waitForEnd(watcher, originalSetTimeout);
          for (let value = 2; value <= 4; value += 1) {
            await delay(1100);
            const nextEnd = waitForEnd(watcher, originalSetTimeout);
            fs.writeFileSync(input, 'export const value = ' + value + ';');
            await nextEnd;
          }
          await cancellationsObserved;
          const evictionDeadline = Date.now() + 10_000;
          while (
            binding.isCurrentThreadHostRegistrationActive(registration.high, registration.low)
          ) {
            if (Date.now() >= evictionDeadline) {
              throw new Error('timer host was not evicted after three cancellation failures');
            }
            await new Promise((resolve) => originalSetTimeout(resolve, 10));
          }
          await watcher.close();
          const stopRuntime = binding.__rolldownTestStopAsyncRuntime;
          const startRuntime = binding.__rolldownTestStartAsyncRuntime;
          if (typeof stopRuntime !== 'function' || typeof startRuntime !== 'function') {
            throw new Error('runtime lifecycle test exports are unavailable');
          }
          stopRuntime();
          startRuntime();
          binding.unregisterTimerHost(registration.high, registration.low);
          console.log(JSON.stringify({
            cancellationCount,
            hostEvicted: true,
            runtimeRestarted: true,
          }));
        } finally {
          await watcher.close();
          binding.unregisterTimerHost(registration.high, registration.low);
          fs.rmSync(directory, { force: true, recursive: true });
        }
      `);

      expect(child.error, child.stderr).toBeUndefined();
      expect(child.status, child.stderr).toBe(0);
      expect(child.stderr).toContain('host timer cancellation callback failed');
      expect(child.stderr).toContain('this relay failure was already accounted');
      expect(child.stderr).toContain('3 times in a row');
      const lines = child.stdout.trim().split('\n');
      const result = JSON.parse(lines[lines.length - 1]);
      expect(result).toMatchObject({
        hostEvicted: true,
        runtimeRestarted: true,
      });
      expect(result.cancellationCount).toBeGreaterThanOrEqual(3);
    },
  );

  test(
    'repeated timer-host unregister retires the exact registration idempotently',
    { timeout: 50_000 },
    () => {
      const child = runCurrentThreadChild(`
        ${childPrelude}
        const binding = await loadBinding();
        let retiredHostArms = 0;
        const registration = binding.reserveCurrentThreadHostRegistration();
        binding.registerTimerHost(
          registration.high,
          registration.low,
          () => {
            retiredHostArms += 1;
            return Promise.resolve();
          },
          () => {},
        );
        binding.unregisterTimerHost(registration.high, registration.low);
        binding.unregisterTimerHost(registration.high, registration.low);

        const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'rd-host-unregister-'));
        const input = path.join(directory, 'main.js');
        fs.writeFileSync(input, 'export const value = 1;');
        const watcher = watch({
          input,
          cwd: directory,
          output: { dir: path.join(directory, 'dist') },
          watch: {
            buildDelay: 175,
            watcher: { usePolling: true, pollInterval: 25 },
          },
        });
        const originalSetTimeout = globalThis.setTimeout;
        try {
          await waitForEnd(watcher, originalSetTimeout);
          await delay(1100);
          const secondEnd = waitForEnd(watcher, originalSetTimeout);
          fs.writeFileSync(input, 'export const value = 2;');
          await secondEnd;
          console.log(JSON.stringify({ retiredHostArms }));
        } finally {
          await watcher.close();
          fs.rmSync(directory, { force: true, recursive: true });
        }
      `);

      expect(child.error, child.stderr).toBeUndefined();
      expect(child.status, child.stderr).toBe(0);
      const lines = child.stdout.trim().split('\n');
      expect(JSON.parse(lines[lines.length - 1])).toEqual({ retiredHostArms: 0 });
    },
  );
});
