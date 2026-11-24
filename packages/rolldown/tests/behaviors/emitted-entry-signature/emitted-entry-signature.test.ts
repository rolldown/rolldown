import { execFileSync } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { rolldown } from 'rolldown';
import { describe, expect, test } from 'vitest';
import { getOutputChunk } from '../../src/utils';

const cwd = import.meta.dirname;
const selection = path.join(cwd, 'selection.mjs');

function check(appFile: string, publicFile: string, mode: string) {
  execFileSync(process.execPath, [path.join(cwd, 'check.mjs'), appFile, publicFile, mode], {
    encoding: 'utf8',
  });
}

test('the unbundled selection entry exposes live aliases', () => {
  check(path.join(cwd, 'app.mjs'), selection, 'internal');
});

describe.each([
  { mode: 'internal', input: 'app.mjs' },
  { mode: 'public-only', input: 'store.mjs' },
])('emitted allow-extension entry ($mode)', ({ mode, input }) => {
  test.each(['es', 'cjs'] as const)('keeps its public aliases in %s', async (format) => {
    const dir = await mkdtemp(path.join(tmpdir(), 'rolldown-selection-'));
    let referenceId: string;
    let publicFile: string | undefined;
    try {
      await using bundle = await rolldown({
        cwd,
        input,
        plugins: [
          {
            name: 'publish-selection',
            buildStart() {
              referenceId = this.emitFile({
                type: 'chunk',
                id: './selection.mjs',
                preserveSignature: 'allow-extension',
              });
            },
            generateBundle() {
              publicFile = this.getFileName(referenceId);
            },
          },
        ],
      });
      const ext = format === 'es' ? 'mjs' : 'cjs';
      const chunks = getOutputChunk(
        await bundle.write({
          dir,
          format,
          entryFileNames: `[name]-[hash].${ext}`,
          chunkFileNames: `[name]-[hash].${ext}`,
          codeSplitting: {
            includeDependenciesRecursively: false,
            groups: [{ name: 'views', test: /[\\/]selection\.mjs$/ }],
          },
        }),
      );
      const app = chunks.find((chunk) => path.basename(chunk.facadeModuleId ?? '') === input)!;
      const published = chunks.find((chunk) => chunk.fileName === publicFile);
      expect.soft(published, `getFileName() returned ${publicFile}`).toBeDefined();
      // Check the signature independently when getFileName() points to a missing file.
      const provider =
        published ??
        chunks.find((chunk) => path.basename(chunk.facadeModuleId ?? '') === 'selection.mjs') ??
        chunks.find((chunk) => chunk.moduleIds.some((id) => path.basename(id) === 'selection.mjs'));
      expect(provider).toBeDefined();
      check(path.join(dir, app.fileName), path.join(dir, provider!.fileName), mode);
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });
});
