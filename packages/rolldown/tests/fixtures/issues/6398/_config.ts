import nodePath from 'node:path';
import { defineTest } from 'rolldown-tests';

export default defineTest({
  config: {
    external: ['node:assert'],
    plugins: [
      {
        name: 'test',
        async resolveId(specifier, _importer, _extraArgs) {
          if (specifier === 'dep') {
            return {
              id: nodePath.resolve(import.meta.dirname, 'node_modules/dep/lib.js'),
              packageJsonPath: nodePath.resolve(
                import.meta.dirname,
                'node_modules/dep/package.json',
              ),
            };
          }
        },
      },
    ],
  },
  async afterTest() {
    // @ts-ignore
    await import('./dist/main');
  },
});
