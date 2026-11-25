import path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

// `manualChunks` becomes a group without `test` and takes no module here, so the fold moves
// `shared.js` into `main.js` as without it. The common chunk merge is off, so only the fold can
// move it.
export default defineTest({
  config: {
    input: path.join(__dirname, 'main.js'),
    preserveEntrySignatures: 'allow-extension',
    experimental: { chunkOptimization: { mergeCommonChunks: false } },
    output: {
      manualChunks: (id) => (id.includes('node_modules') ? 'vendor' : null),
    },
  },
  async afterTest(output) {
    const fileNames = output.output.map((file) => file.fileName);
    expect(fileNames.some((fileName) => fileName.startsWith('shared'))).toBe(false);
    const log: string[] = [];
    (globalThis as any).foldManualChunksLog = log;
    const main = await import('./dist/main.js' as string);
    expect(log).toEqual(['shared', 'main:shared']);
    expect((await main.load()).dynamic).toBe('shared');
  },
});
