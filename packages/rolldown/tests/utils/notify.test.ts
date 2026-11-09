import { once } from 'node:events';
import { mkdtemp, realpath, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { chokidar, FSWatcher, watch, Watcher } from 'rolldown/utils';
import { expect, onTestFinished, test, vi } from 'vitest';

test('watch files through the native binding', async () => {
  const root = await realpath(await mkdtemp(path.join(tmpdir(), 'rolldown-notify-')));
  const file = path.join(root, 'entry.js');
  const onEvent = vi.fn();
  const watcher = watch(root, onEvent, { usePolling: true, pollInterval: 20 });
  onTestFinished(async () => {
    watcher.close();
    await rm(root, { recursive: true, force: true });
  });

  expect(watcher).toBeInstanceOf(Watcher);
  expect(watcher.isWatched(file)).toBe(true);

  await writeFile(file, 'export const value = 1;');
  await expect
    .poll(() => onEvent, { timeout: 5000 })
    .toHaveBeenCalledWith(expect.objectContaining({ kind: 'add', path: file }));

  watcher.remove(file);
  expect(watcher.isWatched(file)).toBe(false);
  watcher.add(file);
  expect(watcher.isWatched(file)).toBe(true);

  watcher.close();
  expect(watcher.closed).toBe(true);
  expect(watcher.isWatched(file)).toBe(false);
});

test('Chokidar watches files through the native binding', async () => {
  const root = await realpath(await mkdtemp(path.join(tmpdir(), 'rolldown-chokidar-')));
  const watcher = chokidar.watch('.', { cwd: root, usePolling: true, pollInterval: 20 });
  const onAdd = vi.fn();
  const onAll = vi.fn();
  watcher.on('add', onAdd).on('all', onAll);
  onTestFinished(async () => {
    watcher.close();
    await rm(root, { recursive: true, force: true });
  });

  expect(watcher).toBeInstanceOf(FSWatcher);
  await once(watcher, 'ready');
  await writeFile(path.join(root, 'entry.js'), 'export const value = 1;');
  await expect
    .poll(() => onAdd, { timeout: 5000 })
    .toHaveBeenCalledWith('entry.js', expect.anything());
  await expect
    .poll(() => onAll, { timeout: 5000 })
    .toHaveBeenCalledWith('add', 'entry.js', expect.anything());

  watcher.unwatch('entry.js');
  expect(watcher.isWatched('entry.js')).toBe(false);
  watcher.add('entry.js');
  expect(watcher.isWatched('entry.js')).toBe(true);

  await watcher.close();
  expect(watcher.closed).toBe(true);
  expect(watcher.listenerCount('all')).toBe(0);
});
