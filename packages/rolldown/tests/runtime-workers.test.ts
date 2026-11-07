import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { isWasiTest, testsDir } from 'rolldown-tests/utils';
import { describe, expect, test } from 'vitest';

// Native runtime workers have Linux thread names; WASI uses a different runtime.
describe.skipIf(process.platform !== 'linux' || isWasiTest)('native runtime workers', () => {
  function countWorkers(env: NodeJS.ProcessEnv = {}, cpu?: string): number {
    const script = `
      import { readdirSync, readFileSync } from 'node:fs';
      import { setTimeout } from 'node:timers/promises';
      const { rolldown } = await import('rolldown');
      // The worker pool starts on the first async binding call.
      const bundle = await rolldown({
        input: 'entry',
        plugins: [{
          name: 'virtual',
          resolveId: (id) => (id === 'entry' ? id : null),
          load: (id) => (id === 'entry' ? 'export default 1' : null),
        }],
      });
      await bundle.generate();
      // Let newly created threads finish setting their names before inspecting /proc.
      await setTimeout(100);
      const workers = readdirSync('/proc/self/task').filter(tid => {
        try {
          // Linux keeps 15 bytes of a thread name: 'rolldown-runtime-N' reads as 'rolldown-runtim'.
          // The timer thread shares that prefix but starts only for watch mode.
          return readFileSync('/proc/self/task/' + tid + '/comm', 'utf8').trim() === 'rolldown-runtim';
        } catch (error) {
          // Other threads can exit between listing tasks and reading their names.
          if (error.code === 'ENOENT') return false;
          throw error;
        }
      });
      console.log(workers.length);
    `;
    const args = ['--input-type=module', '--eval', script];
    const output = execFileSync(
      cpu === undefined ? process.execPath : 'taskset',
      cpu === undefined ? args : ['--cpu-list', cpu, process.execPath, ...args],
      {
        cwd: testsDir(),
        encoding: 'utf8',
        timeout: 10_000,
        env: {
          ...process.env,
          ROLLDOWN_WORKER_THREADS: undefined,
          TOKIO_WORKER_THREADS: undefined,
          ...env,
        },
      },
    );
    return Number(output.trim());
  }

  test('honors TOKIO_WORKER_THREADS', () => {
    expect(countWorkers({ TOKIO_WORKER_THREADS: '2' })).toBe(2);
  });

  test('prefers the Rolldown override', () => {
    expect(countWorkers({ ROLLDOWN_WORKER_THREADS: '3', TOKIO_WORKER_THREADS: '2' })).toBe(3);
  });

  test.each(['0', 'invalid'])('ignores invalid Rolldown override %s', (value) => {
    expect(countWorkers({ ROLLDOWN_WORKER_THREADS: value, TOKIO_WORKER_THREADS: '2' })).toBe(2);
  });

  test('uses CPU affinity when choosing the default', () => {
    // Select a CPU the parent can use; CI containers need not have CPU 0 available.
    const allowed = readFileSync('/proc/self/status', 'utf8').match(/^Cpus_allowed_list:\s*(\d+)/m);
    expect(allowed).not.toBeNull();
    // 1.5x one CPU rounds down to 1; MultiThread keeps at least two workers.
    expect(countWorkers({}, allowed![1])).toBe(2);
    // Explicit overrides remain authoritative even when the process has only one CPU.
    expect(countWorkers({ ROLLDOWN_WORKER_THREADS: '3' }, allowed![1])).toBe(3);
  });
});
