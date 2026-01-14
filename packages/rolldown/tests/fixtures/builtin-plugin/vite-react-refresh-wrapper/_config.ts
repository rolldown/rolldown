import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';
import { viteReactRefreshWrapperPlugin } from 'rolldown/experimental';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export default defineTest({
  config: {
    input: './main.jsx',
    external: ['/@react-refresh'],
    plugins: [
      viteReactRefreshWrapperPlugin({
        cwd: path.dirname(fileURLToPath(import.meta.url)),
        include: [/\.[jt]sx?$/],
        exclude: [],
        jsxImportSource: 'react',
        reactRefreshHost: '',
      }),
    ],
  },
  afterTest: (output) => {
    // Verify the transform succeeded and includes refresh wrapper code
    // This test would fail with "Missing field moduleType" before the fix
    const code = output.output[0].code;
    expect(code).toContain('RefreshRuntime');
    expect(code).toContain('import.meta.hot');
  },
});
