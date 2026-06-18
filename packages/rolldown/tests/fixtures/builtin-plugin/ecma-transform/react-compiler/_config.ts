import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { viteTransformPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: './main.jsx',
    plugins: [
      viteTransformPlugin({
        root: __dirname,
        transformOptions: { reactCompiler: { target: '18' } },
      }),
    ],
  },
  afterTest: (output) => {
    const chunk = getOutputChunk(output)[0];
    // The vite-transform plugin threads `target: '18'` through to the React
    // Compiler: the memo cache helper comes from the `react-compiler-runtime`
    // shim (not `react/compiler-runtime`), and `_c(n)` proves it ran.
    expect(chunk.code).toContain('react-compiler-runtime');
    expect(chunk.code).not.toContain('react/compiler-runtime');
    expect(chunk.code).toMatch(/\b_?c\(\d+\)/);
  },
});
