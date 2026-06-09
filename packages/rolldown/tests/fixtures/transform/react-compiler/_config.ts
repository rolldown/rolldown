import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: 'main.jsx',
    external: ['react', 'react/jsx-runtime', 'react/compiler-runtime'],
    transform: {
      reactCompiler: true,
      jsx: 'react-jsx',
    },
  },
  afterTest: (output) => {
    const chunk = getOutputChunk(output)[0];
    // The React Compiler runs before JSX lowering: it imports the memo cache
    // helper from `react/compiler-runtime` and wraps the component body in
    // memoization guards, while the JSX is still lowered to `jsx(...)` afterwards.
    expect(chunk.code.includes('react/compiler-runtime')).toBe(true);
    expect(chunk.code.includes('react/jsx-runtime')).toBe(true);
    expect(chunk.code).toMatch(/\b_?c\(\d+\)/);
  },
});
