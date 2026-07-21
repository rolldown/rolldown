import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const seen: Record<string, string>[] = [];

export default defineTest({
  config: {
    output: {
      chunkFileNames: 'nested/[name].js',
    },
    plugins: [
      {
        name: 'emit-chunk',
        load(id) {
          if (!id.endsWith('main.js')) return;
          const referenceId = this.emitFile({
            type: 'chunk',
            id: path.join(import.meta.dirname, 'worklet.js'),
          });
          return `export const url = import.meta.ROLLUP_FILE_URL_${referenceId};`;
        },
      },
      {
        name: 'resolve-file-url',
        resolveFileUrl(args) {
          seen.push({ ...args });
          return `'chunk:' + ${JSON.stringify(args.relativePath)}`;
        },
      },
    ],
  },
  afterTest: async (output) => {
    // Emitted chunks are referenced exactly like emitted assets.
    expect(seen).toHaveLength(1);
    const args = seen[0];
    expect(args.fileName).toBe('nested/worklet.js');
    // The importing chunk sits at the output root, so the path descends into `nested/`.
    expect(args.relativePath).toBe('nested/worklet.js');
    expect(args.chunkId).toBe('main.js');

    expect(output.output.some((o) => o.fileName === 'nested/worklet.js')).toBe(true);
    const chunk = output.output.find((o) => o.fileName === 'main.js')!;
    expect(chunk.type).toBe('chunk');
    expect(chunk.type === 'chunk' && chunk.code).toContain('"chunk:nested/worklet.js"');
  },
});
