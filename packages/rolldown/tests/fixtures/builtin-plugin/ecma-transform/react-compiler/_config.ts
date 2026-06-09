import { defineTest } from 'rolldown-tests';
import { getOutputChunk } from 'rolldown-tests/utils';
import { viteTransformPlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: './main.jsx',
    external: ['react', 'react/jsx-runtime', 'react/compiler-runtime'],
    plugins: [
      viteTransformPlugin({
        root: __dirname,
        // `jsx` is left at its default (automatic runtime). Unlike the bundler's
        // own transform options, the vite-transform plugin only accepts `preserve`
        // as a string preset; Vite itself passes a resolved jsx options object.
        transformOptions: { reactCompiler: true },
      }),
    ],
  },
  afterTest: (output) => {
    const chunk = getOutputChunk(output)[0];
    // The vite-transform plugin runs the React Compiler before JSX lowering, same
    // as the bundler's `pre_process_ecma_ast` pass: it imports the memo cache helper
    // from `react/compiler-runtime` and wraps the component body in memoization
    // guards, while the JSX is still lowered to `jsx(...)` afterwards.
    expect(chunk.code.includes('react/compiler-runtime')).toBe(true);
    expect(chunk.code.includes('react/jsx-runtime')).toBe(true);
    expect(chunk.code).toMatch(/\b_?c\(\d+\)/);
  },
});
