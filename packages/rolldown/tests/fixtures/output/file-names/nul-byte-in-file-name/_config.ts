import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    output: {
      chunkFileNames: (chunk) => {
        const path = chunk.facadeModuleId || chunk.moduleIds.at(-1);
        return `${path}.[hash].js`;
      },
    },
    plugins: [
      {
        name: 'virtual-plugin',
        resolveId(id) {
          if (id === 'virtual:my-module') {
            return '\0virtual:my-module';
          }
        },
        load(id) {
          if (id === '\0virtual:my-module') {
            return `export const data = "hello from virtual module";`;
          }
        },
      },
    ],
  },
  catchError(err) {
    expect((err as Error).message).toContain('null byte');
  },
});
