import nodePath from 'node:path';
import { defineTest } from 'rolldown-tests';
import { viteResolvePlugin } from 'rolldown/experimental';
import { expect, vi } from 'vitest';

const fn = vi.fn();

const slash = (value: string) => value.replaceAll('\\', '/');

export default defineTest({
  sequential: true,
  config: {
    plugins: [
      {
        name: 'test-plugin-context',
        async buildStart(this) {
          const importer = nodePath.join(import.meta.dirname, 'entry.scss');
          const ret = await this.resolve(
            'sass-pkg-with-wildcard-partial/styles/mixins',
            importer,
          );
          if (!ret) {
            throw new Error('resolve failed');
          }
          expect(slash(ret.id)).toBe(
            slash(
              nodePath.join(
                import.meta.dirname,
                'node_modules/sass-pkg-with-wildcard-partial/dist/styles/_mixins.scss',
              ),
            ),
          );
          fn();
        },
      },
      viteResolvePlugin({
        resolveOptions: {
          isBuild: true,
          isProduction: true,
          asSrc: false,
          preferRelative: true,
          root: import.meta.dirname,
          scan: false,
          mainFields: ['sass', 'style'],
          conditions: ['sass', 'style', 'production'],
          externalConditions: [],
          extensions: ['.scss', '.sass', '.css'],
          tryIndex: true,
          tryPrefix: '_',
          preserveSymlinks: false,
          tsconfigPaths: false,
        },
        environmentConsumer: 'client',
        environmentName: 'test',
        builtins: [],
        external: [],
        noExternal: [],
        dedupe: [],
        legacyInconsistentCjsInterop: false,
        resolveSubpathImports() {
          throw new Error('Not implemented');
        },
      }),
    ],
  },
  afterTest: () => {
    expect(fn).toHaveBeenCalledTimes(1);
  },
});
