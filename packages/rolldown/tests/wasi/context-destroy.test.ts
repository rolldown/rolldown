import { execFileSync } from 'node:child_process';
import { isWasiTest, testsDir } from 'rolldown-tests/utils';
import { test } from 'vitest';
import { getBindingPath } from '../src/load-binding';

// Ported from #10268: exercise raw Context.destroy() without the runtime host tests.
test.skipIf(!isWasiTest)(
  'fresh WASI contexts prepare and tear down exactly once after raw destruction',
  () => {
    // Keep the real emnapi contexts, workers, and process exit listeners isolated.
    execFileSync(
      process.execPath,
      [
        '--eval',
        `
          const assert = require('node:assert/strict');
          const { createRequire } = require('node:module');
          const bindingPath = process.argv[1];
          const bindingRequire = createRequire(bindingPath);
          const emnapi = bindingRequire('@emnapi/runtime');
          const wasm = bindingRequire('@napi-rs/wasm-runtime');
          const createContext = emnapi.createContext;
          const instantiate = wasm.instantiateNapiModuleSync;
          const contexts = [];
          const cleanups = [];
          const events = [];

          for (const id of [1, 2]) {
            emnapi.createContext = (options) => {
              const context = createContext(options);
              context.addCleanupHook(undefined, () => events.push('teardown:' + id), 0);
              contexts.push(context);
              return context;
            };
            wasm.instantiateNapiModuleSync = (...args) => {
              const result = instantiate(...args);
              const exports = result.instance.exports;
              assert.equal(typeof exports.napi_prepare_wasm_env_cleanup, 'function');
              // WebAssembly exports are read-only. Decorate the loader's view while
              // still invoking the real artifact's cleanup preparation.
              return {
                ...result,
                instance: {
                  exports: {
                    ...exports,
                    napi_prepare_wasm_env_cleanup() {
                      events.push('prepare:' + id);
                      return exports.napi_prepare_wasm_env_cleanup();
                    },
                  },
                },
              };
            };
            const previousListeners = new Set(process.listeners('exit'));
            delete bindingRequire.cache[bindingPath];
            bindingRequire(bindingPath);
            const listeners = process.listeners('exit').filter(fn => !previousListeners.has(fn));
            assert.equal(listeners.length, 1);
            cleanups.push(listeners[0]);
          }

          assert.equal(contexts.length, 2);
          assert.notEqual(contexts[0], contexts[1]);
          assert.deepEqual(events, []);
          for (const [index, context] of contexts.entries()) {
            // Bypass the loader's cleanup entry point, as an embedder can.
            context.destroy();
            assert.deepEqual(events, index === 0
              ? ['prepare:1', 'teardown:1']
              : ['prepare:1', 'teardown:1', 'prepare:2', 'teardown:2']);
            context.destroy();
            // Exit-time delegation must share the preparation latch and encounter
            // an already-drained emnapi cleanup queue.
            cleanups[index]();
            cleanups[index]();
          }
          assert.deepEqual(events, ['prepare:1', 'teardown:1', 'prepare:2', 'teardown:2']);
        `,
        getBindingPath(),
      ],
      {
        cwd: testsDir(),
        encoding: 'utf8',
        timeout: 30_000,
        env: {
          ...process.env,
          NAPI_RS_ASYNC_WORK_POOL_SIZE: '1',
          ROLLDOWN_WORKER_THREADS: '1',
        },
      },
    );
  },
  40_000,
);
