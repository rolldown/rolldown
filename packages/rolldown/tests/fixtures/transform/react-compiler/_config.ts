import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: 'main.jsx',
    transform: {
      reactCompiler: { target: '18' },
      jsx: 'react-jsx',
    },
  },
  afterTest: (output) => {
    const chunk = getOutputChunk(output)[0];
    // The bundler threads `target: '18'` through to the React Compiler: the memo
    // cache helper is imported from the `react-compiler-runtime` shim (not React
    // 19's built-in `react/compiler-runtime`), and `_c(n)` proves it ran.
    expect(chunk.code).toContain('react-compiler-runtime');
    expect(chunk.code).not.toContain('react/compiler-runtime');
    expect(chunk.code).toMatch(/\b_?c\(\d+\)/);
  },
});
