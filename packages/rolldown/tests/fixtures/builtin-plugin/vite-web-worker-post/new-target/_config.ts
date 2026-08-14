import { defineTest } from 'rolldown-tests';
import { viteWebWorkerPostPlugin } from 'rolldown/experimental';
import type { OutputChunk } from 'rolldown';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: './main.js',
    plugins: [viteWebWorkerPostPlugin()],
  },
  afterTest(output) {
    const chunk = output.output.find(
      (item): item is OutputChunk => item.type === 'chunk' && item.name === 'main',
    );
    expect(chunk).toBeDefined();
    const code = chunk!.code;
    // new.target must be preserved, not replaced with _vite_importMeta
    expect(code).toContain('new.target');
    expect(code).not.toContain('_vite_importMeta.prototype');
    // import.meta should still be transformed
    expect(code).not.toContain('import.meta');
  },
});
