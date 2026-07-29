import fs from 'node:fs';
import { access, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';

const { close, rolldown, write } = vi.hoisted(() => ({
  close: vi.fn(),
  rolldown: vi.fn(),
  write: vi.fn(),
}));

vi.mock('@src/api/rolldown', () => ({ rolldown }));

import { loadConfig } from '@src/utils/load-config';

const fixture = path.join(
  import.meta.dirname,
  'fixtures',
  'load-config',
  'bundled-cleanup.config.ts',
);
const fixtureDir = path.dirname(fixture);
let generatedFiles: string[] = [];

afterEach(async () => {
  vi.restoreAllMocks();
  close.mockReset();
  rolldown.mockReset();
  write.mockReset();
  for (const generatedFile of generatedFiles) {
    await rm(generatedFile, { force: true });
  }
  generatedFiles = [];
});

async function emit(outputDir: string, fileName: string, code: string): Promise<void> {
  const generatedFile = path.join(outputDir, fileName);
  generatedFiles.push(generatedFile);
  await writeFile(generatedFile, code);
}

describe('loadConfig bundle cleanup', () => {
  it('closes the transient build and removes every generated output', async () => {
    rolldown.mockResolvedValue({ close, write });
    write.mockImplementation(async (outputOptions: { dir: string }) => {
      const fileName = 'rolldown.config.cleanup.mjs';
      await emit(
        outputOptions.dir,
        fileName,
        'import input from "./config-chunk.mjs"; export default { input }',
      );
      await emit(outputOptions.dir, 'config-chunk.mjs', 'export default "./entry.js"');
      await emit(outputOptions.dir, 'config-asset.txt', 'temporary config asset');
      return {
        output: [
          { fileName, isEntry: true, type: 'chunk' },
          { fileName: 'config-chunk.mjs', isEntry: false, type: 'chunk' },
          { fileName: 'config-asset.txt', type: 'asset' },
        ],
      };
    });

    await expect(loadConfig(fixture)).resolves.toStrictEqual({ input: './entry.js' });
    expect(write).toHaveBeenCalledWith(
      expect.objectContaining({
        codeSplitting: false,
        // Runtime-relative resolution inside a config resolves against the
        // directory the generated file lives in, so it has to be the config's own.
        dir: fixtureDir,
      }),
    );
    expect(close).toHaveBeenCalledOnce();
    for (const generatedFile of generatedFiles) {
      await expect(access(generatedFile)).rejects.toMatchObject({ code: 'ENOENT' });
    }
    // The config's own directory must survive the cleanup.
    await expect(access(fixture)).resolves.toBeUndefined();
  });

  it('closes the transient build when config generation fails', async () => {
    const writeError = new Error('config write failed');
    rolldown.mockResolvedValue({ close, write });
    write.mockRejectedValue(writeError);

    const error = await loadConfig(fixture).catch((error: unknown) => error);

    expect((error as Error).cause).toBe(writeError);
    expect(close).toHaveBeenCalledOnce();
  });

  it('preserves both config generation and cleanup failures', async () => {
    const writeError = new Error('config write failed');
    const closeError = new Error('config close failed');
    rolldown.mockResolvedValue({ close, write });
    write.mockRejectedValue(writeError);
    close.mockRejectedValue(closeError);

    const error = await loadConfig(fixture).catch((error: unknown) => error);
    const cause = (error as Error & { cause?: unknown }).cause;

    expect(cause).toBeInstanceOf(AggregateError);
    expect((cause as AggregateError).errors).toEqual([writeError, closeError]);
    expect(close).toHaveBeenCalledOnce();
  });

  it('removes the generated config when close fails after a successful write', async () => {
    const closeError = new Error('config close failed');
    const fileName = 'rolldown.config.close-failure.mjs';
    rolldown.mockResolvedValue({ close, write });
    write.mockImplementation(async (outputOptions: { dir: string }) => {
      await emit(outputOptions.dir, fileName, 'export default {}');
      return {
        output: [{ fileName, isEntry: true, type: 'chunk' }],
      };
    });
    close.mockRejectedValue(closeError);

    const error = await loadConfig(fixture).catch((error: unknown) => error);

    expect((error as Error).cause).toBe(closeError);
    await expect(access(path.join(fixtureDir, fileName))).rejects.toMatchObject({ code: 'ENOENT' });
  });

  it('preserves config import and cleanup failures', async () => {
    const cleanupError = new Error('config cleanup failed');
    const fileName = 'rolldown.config.import-failure.mjs';
    rolldown.mockResolvedValue({ close, write });
    write.mockImplementation(async (outputOptions: { dir: string }) => {
      await emit(outputOptions.dir, fileName, 'throw new Error("config import failed")');
      return {
        output: [{ fileName, isEntry: true, type: 'chunk' }],
      };
    });
    vi.spyOn(fs.promises, 'rm').mockRejectedValueOnce(cleanupError);

    const error = await loadConfig(fixture).catch((error: unknown) => error);
    const cause = (error as Error & { cause?: unknown }).cause;

    expect(cause).toBeInstanceOf(AggregateError);
    expect((cause as AggregateError).errors).toHaveLength(2);
    expect((cause as AggregateError).errors[0]).toMatchObject({ message: 'config import failed' });
    expect((cause as AggregateError).errors[1]).toBe(cleanupError);
  });

  it('rejects when the bundled config rejects with `undefined`', async () => {
    const fileName = 'rolldown.config.undefined-rejection.mjs';
    rolldown.mockResolvedValue({ close, write });
    write.mockImplementation(async (outputOptions: { dir: string }) => {
      await emit(outputOptions.dir, fileName, 'throw undefined');
      return {
        output: [{ fileName, isEntry: true, type: 'chunk' }],
      };
    });

    const error = await loadConfig(fixture).catch((error: unknown) => error);

    expect(error).toBeInstanceOf(Error);
    expect((error as Error & { cause?: unknown }).cause).toBeUndefined();
    await expect(access(path.join(fixtureDir, fileName))).rejects.toMatchObject({ code: 'ENOENT' });
  });

  it('rejects when the config build rejects with `undefined`', async () => {
    rolldown.mockResolvedValue({ close, write });
    write.mockRejectedValue(undefined);

    const error = await loadConfig(fixture).catch((error: unknown) => error);

    expect(error).toBeInstanceOf(Error);
    expect((error as Error & { cause?: unknown }).cause).toBeUndefined();
    expect(close).toHaveBeenCalledOnce();
  });
});
