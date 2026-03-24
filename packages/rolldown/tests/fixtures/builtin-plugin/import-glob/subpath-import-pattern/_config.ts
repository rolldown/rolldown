import * as fs from 'node:fs';
import * as path from 'node:path';
import { defineTest } from 'rolldown-tests';
import { viteImportGlobPlugin, viteResolvePlugin } from 'rolldown/experimental';
import { expect } from 'vitest';

const root = import.meta.dirname;
const packageJson = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8')) as {
  imports?: Record<string, string>;
};

// Mirrors Vite's resolveSubpathImports callback contract:
// https://github.com/vitejs/vite/blob/v8.0.0/packages/vite/src/node/plugins/resolve.ts#L491-L520
function resolveSubpathImports(id: string, importer?: string) {
  if (!importer || !id.startsWith('#src/')) {
    return;
  }

  const target = packageJson.imports?.['#src/*'];
  if (!target) {
    return;
  }

  const suffix = id.slice('#src/'.length);
  const resolvedTarget = path.join(root, target.replace('*', suffix).replace(/^\.\//, ''));
  let relativeTarget = path.relative(path.dirname(importer), resolvedTarget).replaceAll(path.sep, '/');
  if (!relativeTarget.startsWith('./') && !relativeTarget.startsWith('../')) {
    relativeTarget = `./${relativeTarget}`;
  }
  return relativeTarget;
}

export default defineTest({
  config: {
    input: 'src/main.ts',
    output: {
      entryFileNames: '[name].js',
      chunkFileNames: '[name].js',
      format: 'esm',
    },
    plugins: [
      viteResolvePlugin({
        resolveOptions: {
          isBuild: true,
          isProduction: true,
          asSrc: false,
          preferRelative: false,
          root,
          scan: false,
          mainFields: ['module', 'main'],
          conditions: [],
          externalConditions: [],
          extensions: ['.mjs', '.js', '.ts', '.json'],
          tryIndex: true,
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
        resolveSubpathImports,
      }),
      viteImportGlobPlugin({ root }),
    ],
  },
  async afterTest(output) {
    const mainChunk = output.output.find(
      (chunk) => chunk.type === 'chunk' && chunk.fileName === 'main.js',
    );
    expect(mainChunk && 'code' in mainChunk ? mainChunk.code : '').toContain(
      '"/src/baz/bar.ts": () => import(',
    );
    expect(mainChunk && 'code' in mainChunk ? mainChunk.code : '').toContain(
      '"/src/baz/foo.ts": () => import(',
    );
  },
});
