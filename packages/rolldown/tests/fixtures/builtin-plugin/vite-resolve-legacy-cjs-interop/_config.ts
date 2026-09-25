import nodePath from 'node:path';
import { defineTest } from 'rolldown-tests';
import { viteResolvePlugin } from 'rolldown/experimental';
import { expect, vi } from 'vitest';

const fn = vi.fn();

type CallableResolver = {
  resolveId(
    id: string,
    importer?: string,
  ): Promise<{ id: string; skipPackageJsonLookup: boolean } | null | undefined>;
};

function resolver(legacyInconsistentCjsInterop: boolean): CallableResolver {
  return viteResolvePlugin({
    resolveOptions: {
      isBuild: true,
      isProduction: true,
      asSrc: false,
      preferRelative: false,
      root: import.meta.dirname,
      scan: false,
      mainFields: ['main'],
      conditions: [],
      externalConditions: [],
      extensions: ['.js'],
      tryIndex: false,
      preserveSymlinks: false,
      tsconfigPaths: false,
    },
    environmentConsumer: 'client',
    environmentName: 'test',
    builtins: [],
    external: [],
    noExternal: [],
    dedupe: [],
    legacyInconsistentCjsInterop,
    resolveSubpathImports() {
      throw new Error('Not implemented');
    },
  }) as unknown as CallableResolver;
}

export default defineTest({
  sequential: true,
  config: {
    plugins: [
      {
        name: 'test-callable-resolver',
        async buildStart() {
          const importer = nodePath.join(import.meta.dirname, 'main.js');
          // The callable form hands the Rust hook's output straight to JS, so the opt-out that
          // `legacyInconsistentCjsInterop` sets has to come through with it.
          const legacy = await resolver(true).resolveId('./target.js', importer);
          expect(legacy?.skipPackageJsonLookup).toBe(true);
          const current = await resolver(false).resolveId('./target.js', importer);
          expect(current?.skipPackageJsonLookup).toBe(false);
          fn();
        },
      },
    ],
  },
  afterTest: () => {
    expect(fn).toHaveBeenCalledTimes(1);
  },
});
