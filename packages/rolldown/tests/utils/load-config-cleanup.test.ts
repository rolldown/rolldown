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

async function emitFile(dir: string, fileName: string, content: string): Promise<void> {
  const filePath = path.join(dir, fileName);
  generatedFiles.push(filePath);
  await writeFile(filePath, content);
}

afterEach(async () => {
  vi.restoreAllMocks();
  close.mockReset();
  rolldown.mockReset();
  write.mockReset();
  await Promise.all(generatedFiles.map((file) => rm(file, { force: true })));
  generatedFiles = [];
});

describe('loadConfig bundle cleanup', () => {
  it('closes the transient build, removes the imported entry, and keeps sibling outputs for deferred imports', async () => {
    rolldown.mockResolvedValue({ close, write });
    let outputDir: string | undefined;
    write.mockImplementation(async (outputOptions: { dir: string }) => {
      outputDir = outputOptions.dir;
      const fileName = 'rolldown.config.cleanup.mjs';
      await emitFile(
        outputOptions.dir,
        fileName,
        'import input from "./config-chunk.mjs"; export default { input }',
      );
      await emitFile(outputOptions.dir, 'config-chunk.mjs', 'export default "./entry.js"');
      await emitFile(outputOptions.dir, 'config-asset.txt', 'temporary config asset');
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
        entryFileNames: expect.stringMatching(/^\.rolldown\.config\.[0-9a-f]{8}\./),
      }),
    );
    expect(close).toHaveBeenCalledOnce();
    // The entry is emitted directly beside the config file so that runtime
    // relative dynamic imports in a deferred config function resolve against
    // the config's own directory.
    expect(outputDir).toBe(fixtureDir);
    // The imported entry is removed immediately (it already lives in memory)…
    await expect(access(path.join(fixtureDir, 'rolldown.config.cleanup.mjs'))).rejects.toMatchObject(
      { code: 'ENOENT' },
    );
    // …while sibling outputs a deferred config function may still import stay
    // on disk until the process exits.
    await expect(access(path.join(fixtureDir, 'config-chunk.mjs'))).resolves.toBeUndefined();
    await expect(access(path.join(fixtureDir, 'config-asset.txt'))).resolves.toBeUndefined();
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
      await emitFile(outputOptions.dir, fileName, 'export default {}');
      return {
        output: [{ fileName, isEntry: true, type: 'chunk' }],
      };
    });
    close.mockRejectedValue(closeError);

    const error = await loadConfig(fixture).catch((error: unknown) => error);

    expect((error as Error).cause).toBe(closeError);
    await expect(access(path.join(fixtureDir, fileName))).rejects.toMatchObject({
      code: 'ENOENT',
    });
  });

  it('preserves config import and cleanup failures', async () => {
    const cleanupError = new Error('config file cleanup failed');
    const fileName = 'rolldown.config.import-failure.mjs';
    rolldown.mockResolvedValue({ close, write });
    write.mockImplementation(async (outputOptions: { dir: string }) => {
      await emitFile(outputOptions.dir, fileName, 'throw new Error("config import failed")');
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
