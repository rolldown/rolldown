import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import type { RolldownWatcherEvent } from 'rolldown';
import { watch } from 'rolldown';
import { test, vi } from 'vitest';

test.for(['options', 'asyncOptions', 'outputOptions', 'native'] as const)(
  'watch emits an ERROR event for a %s setup failure',
  async (stage, { expect, onTestFinished }) => {
    const cwd = fs.mkdtempSync(path.join(os.tmpdir(), 'rolldown-watch-setup-'));
    const input = path.join(cwd, 'main.js');
    const output = path.join(cwd, 'out.js');
    fs.writeFileSync(input, 'console.log(42)');
    const failure = new Error(`${stage} setup failure`);
    const buildStart = vi.fn();
    const watcher = watch({
      input,
      output: { file: output },
      experimental: { devMode: stage === 'native' },
      plugins: [
        {
          name: 'setup-failure',
          options() {
            if (stage === 'options') throw failure;
            if (stage === 'asyncOptions') return Promise.reject(failure);
          },
          outputOptions() {
            if (stage === 'outputOptions') throw failure;
          },
          buildStart,
        },
      ],
    });
    onTestFinished(async () => {
      await watcher.close();
      fs.rmSync(cwd, { recursive: true, force: true });
    });

    const events: RolldownWatcherEvent[] = [];
    watcher.on('event', async (event) => {
      events.push(event);
      if (event.code === 'ERROR') await watcher.close();
    });

    await expect.poll(() => events.length, { timeout: 1000 }).toBe(1);
    const event = events[0];
    expect(event.code).toBe('ERROR');
    if (event.code !== 'ERROR') throw new Error('Expected a setup error');
    expect(event.result).toBeUndefined();
    if (stage === 'native') {
      expect(event.error.message).toContain('"experimental.devMode"');
    } else {
      expect(event.error).toBe(failure);
    }
    expect(buildStart).not.toHaveBeenCalled();
    expect(fs.existsSync(output)).toBe(false);
  },
);

test('watch handles a rejected asynchronous setup error listener', async ({ expect }) => {
  const failure = new Error('options failure');
  const listenerFailure = new Error('listener failure');
  const report = vi.spyOn(console, 'error').mockImplementation(() => {});
  const watcher = watch({
    plugins: [
      {
        name: 'setup-failure',
        options() {
          throw failure;
        },
      },
    ],
  });
  watcher.on('event', async () => {
    throw listenerFailure;
  });
  try {
    await expect
      .poll(() => report.mock.calls)
      .toEqual([['watcher on_event listener error:', listenerFailure]]);
  } finally {
    await watcher.close();
    report.mockRestore();
  }
});
