import type * as Crypto from 'node:crypto';
import fs from 'node:fs';
import { mkdtemp, readdir, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const { close, randomBytes, rolldown, write } = vi.hoisted(() => ({
  close: vi.fn(),
  randomBytes: vi.fn(),
  rolldown: vi.fn(),
  write: vi.fn(),
}));

vi.mock('../../src/api/rolldown', () => ({ rolldown }));
vi.mock('node:crypto', async (importOriginal) => {
  const crypto = await importOriginal<typeof Crypto>();
  randomBytes.mockImplementation(crypto.randomBytes);
  return { ...crypto, randomBytes };
});

import { loadConfig } from '../../src/utils/load-config';

interface WriteOptions {
  dir: string;
  entryFileNames: string;
}

let configDir: string;
let configFile: string;

beforeEach(async () => {
  configDir = await mkdtemp(path.join(os.tmpdir(), 'rolldown-load-config-'));
  configFile = path.join(configDir, 'rolldown.config.mts');
  rolldown.mockResolvedValue({ close, write });
});

afterEach(async () => {
  vi.restoreAllMocks();
  close.mockReset();
  rolldown.mockReset();
  write.mockReset();
  await rm(configDir, { recursive: true, force: true });
});

/** Mocked `bundle.write`: the first file is the entry. */
function emit(files: Record<string, string>) {
  return async ({ dir, entryFileNames }: WriteOptions) => {
    const output = Object.entries(files).map(([fileName, code], index) => ({
      fileName: index === 0 ? entryFileNames : fileName,
      code,
    }));
    for (const { fileName, code } of output) {
      await writeFile(path.join(dir, fileName), code);
    }
    return {
      output: output.map(({ fileName }, index) =>
        index === 0 ? { fileName, isEntry: true, type: 'chunk' } : { fileName, type: 'asset' },
      ),
    };
  };
}

describe('loadConfig bundle cleanup', () => {
  it('closes the build and removes every generated file after the import', async () => {
    write.mockImplementation(
      emit({
        entry: 'export default { input: "./entry.js" }',
        'config-asset.txt': 'temporary config asset',
      }),
    );

    await expect(loadConfig(configFile)).resolves.toStrictEqual({ input: './entry.js' });

    expect(write).toHaveBeenCalledWith(
      expect.objectContaining({ codeSplitting: false, dir: configDir }),
    );
    expect(close).toHaveBeenCalledOnce();
    expect(await readdir(configDir)).toStrictEqual([]);
  });

  it('gives concurrent loads of one config their own entry file', async () => {
    write.mockImplementation(emit({ entry: 'export default {}' }));

    await Promise.all([loadConfig(configFile), loadConfig(configFile)]);

    const [first, second] = write.mock.calls.map(([options]) => options.entryFileNames);
    expect(first).not.toBe(second);
    expect(await readdir(configDir)).toStrictEqual([]);
  });

  it('evaluates every load even when the random part of the name repeats', async () => {
    const repeated = Buffer.alloc(8);
    randomBytes.mockReturnValueOnce(repeated).mockReturnValueOnce(repeated);
    write
      .mockImplementationOnce(emit({ entry: 'export default { load: 1 }' }))
      .mockImplementationOnce(emit({ entry: 'export default { load: 2 }' }));

    await expect(loadConfig(configFile)).resolves.toStrictEqual({ load: 1 });
    await expect(loadConfig(configFile)).resolves.toStrictEqual({ load: 2 });

    const [first, second] = write.mock.calls.map(([options]) => options.entryFileNames);
    expect(first).not.toBe(second);
  });

  it('removes what a failed write left behind and still closes the build', async () => {
    const writeError = new Error('config write failed');
    write.mockImplementation(async (options: WriteOptions) => {
      await emit({ entry: 'export default {}' })(options);
      throw writeError;
    });

    const error = await loadConfig(configFile).catch((error: unknown) => error);

    expect((error as Error).cause).toBe(writeError);
    expect(close).toHaveBeenCalledOnce();
    expect(await readdir(configDir)).toStrictEqual([]);
  });

  it('removes the generated files when close fails after a successful write', async () => {
    const closeError = new Error('config close failed');
    write.mockImplementation(emit({ entry: 'export default {}' }));
    close.mockRejectedValue(closeError);

    const error = await loadConfig(configFile).catch((error: unknown) => error);

    expect((error as Error).cause).toBe(closeError);
    expect(await readdir(configDir)).toStrictEqual([]);
  });

  it('keeps both errors when the write and the close fail', async () => {
    const writeError = new Error('config write failed');
    const closeError = new Error('config close failed');
    write.mockRejectedValue(writeError);
    close.mockRejectedValue(closeError);

    const error = await loadConfig(configFile).catch((error: unknown) => error);
    const cause = (error as Error).cause;

    expect(cause).toBeInstanceOf(AggregateError);
    expect((cause as AggregateError).errors).toStrictEqual([writeError, closeError]);
  });

  it('keeps both errors when the import and the cleanup fail', async () => {
    const cleanupError = new Error('config cleanup failed');
    write.mockImplementation(emit({ entry: 'throw new Error("config import failed")' }));
    vi.spyOn(fs.promises, 'rm').mockRejectedValueOnce(cleanupError);

    const error = await loadConfig(configFile).catch((error: unknown) => error);
    const cause = (error as Error).cause;

    expect(cause).toBeInstanceOf(AggregateError);
    expect((cause as AggregateError).errors).toStrictEqual([
      expect.objectContaining({ message: 'config import failed' }),
      cleanupError,
    ]);
  });

  it('rejects when the write rejects with `undefined`', async () => {
    write.mockRejectedValue(undefined);

    const error = await loadConfig(configFile).catch((error: unknown) => error);

    expect(error).toBeInstanceOf(Error);
    expect((error as Error).cause).toBeUndefined();
    expect(close).toHaveBeenCalledOnce();
  });
});
