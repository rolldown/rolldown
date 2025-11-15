import type { PreRenderedChunk } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

const capturedChunks: PreRenderedChunk[] = [];

export default defineTest({
  config: {
    input: ['src/index.ts'],
    output: {
      preserveModules: true,
      preserveModulesRoot: 'src',
      entryFileNames: (chunk) => {
        capturedChunks.push(chunk);
        return '[name].js';
      },
    },
  },
  afterTest: () => {
    if (process.platform === 'win32') {
      // Skip on Windows due to path differences
      return;
    }

    expect(capturedChunks.length).toBe(3);
    capturedChunks.sort((a, b) => a.name.localeCompare(b.name));

    // Find the button/index chunk

    expect(capturedChunks[0]);
    expect(capturedChunks[0]?.name).toBe('components/button/index');

    // Find the input/index chunk
    expect(capturedChunks[1]).toBeDefined();
    expect(capturedChunks[1]?.name).toBe('components/input/index');

    expect(capturedChunks[2]?.name).toBe('index');
  },
});
