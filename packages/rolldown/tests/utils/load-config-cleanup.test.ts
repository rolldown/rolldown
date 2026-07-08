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
let generatedOutputDir: string | undefined;

afterEach(async () => {
  vi.restoreAllMocks();
  close.mockReset();
  rolldown.mockReset();
  write.mockReset();
  if (generatedOutputDir !== undefined) {
    await rm(generatedOutputDir, { force: true, recursive: true });
    generatedOutputDir = undefined;
  }
});

describe('loadConfig bundle cleanup', () => {
  it('closes the transient build and removes every generated output', async () => {
    rolldown.mockResolvedValue({ close, write });
    write.mockImplementation(async (outputOptions: { dir: string }) => {
      generatedOutputDir = outputOptions.dir;
      const fileName = 'rolldown.config.cleanup.mjs';
      await writeFile(
        path.join(outputOptions.dir, fileName),
        'import input from "./config-chunk.mjs"; export default { input }',
      );
      await writeFile(
        path.join(outputOptions.dir, 'config-chunk.mjs'),
        'export default "./entry.js"',
      );
      await writeFile(path.join(outputOptions.dir, 'config-asset.txt'), 'temporary config asset');
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
      }),
    );
    expect(close).toHaveBeenCalledOnce();
    expect(path.dirname(generatedOutputDir!)).toBe(path.dirname(fixture));
    await expect(access(generatedOutputDir!)).rejects.toMatchObject({ code: 'ENOENT' });
    generatedOutputDir = undefined;
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
      generatedOutputDir = outputOptions.dir;
      await writeFile(path.join(outputOptions.dir, fileName), 'export default {}');
      return {
        output: [{ fileName, isEntry: true, type: 'chunk' }],
      };
    });
    close.mockRejectedValue(closeError);

    const error = await loadConfig(fixture).catch((error: unknown) => error);

    expect((error as Error).cause).toBe(closeError);
    await expect(access(generatedOutputDir!)).rejects.toMatchObject({ code: 'ENOENT' });
    generatedOutputDir = undefined;
  });

  it('preserves config import and recursive cleanup failures', async () => {
    const cleanupError = new Error('config directory cleanup failed');
    const fileName = 'rolldown.config.import-failure.mjs';
    rolldown.mockResolvedValue({ close, write });
    write.mockImplementation(async (outputOptions: { dir: string }) => {
      generatedOutputDir = outputOptions.dir;
      await writeFile(
        path.join(outputOptions.dir, fileName),
        'throw new Error("config import failed")',
      );
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
});
