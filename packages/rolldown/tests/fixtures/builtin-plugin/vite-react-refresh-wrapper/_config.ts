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
      {
        name: 'assert-module-type-updated',
        transform(code, id, meta) {
          if (id.endsWith('main.jsx')) {
            // The vite-react-refresh-wrapper plugin outputs JS code, so moduleType
            // should be 'js', not the original 'jsx' from the file extension.
            // This check catches the bug where HookTransformOutput.module_type is None,
            // causing the moduleType to incorrectly remain as the input type.
            if (code.includes('RefreshRuntime')) {
              expect(meta.moduleType, 'moduleType should be updated to js after transform').toBe('js');
            }
          }
          return null;
        },
      },
    ],
  },
  afterTest: (output) => {
    // Verify the transform succeeded and includes refresh wrapper code
    const code = output.output[0].code;
    expect(code).toContain('RefreshRuntime');
    expect(code).toContain('import.meta.hot');
  },
});
