import * as esbuild from 'esbuild';
import nodePath from 'node:path';
import { defineParallelPluginImplementation } from 'rolldown/parallel-plugin';

/** @returns {import('rolldown').Plugin} */
export const plugin = () => {
  return {
    name: '@rolldown/plugin-esbuild',
    transform(code, id) {
      const ext = nodePath.extname(id);
      if (ext === '.ts' || ext === '.tsx') {
        const now = performance.now();
        const ret = esbuild.transformSync(code, {
          platform: 'node',
          loader: ext === '.tsx' ? 'tsx' : 'ts',
          format: 'esm',
          target: 'chrome80',
          sourcemap: true,
        });
        console.log(
          'esbuild transform time:',
          performance.now() - now,
          nodePath.relative(process.cwd(), id),
        );

        return {
          code: ret.code,
        };
      }
    },
  };
};

/** @returns {import('rolldown').Plugin} */
export const pluginAsync = () => {
  return {
    name: '@rolldown/plugin-esbuild',
    async transform(code, id) {
      const ext = nodePath.extname(id);
      if (ext === '.ts' || ext === '.tsx') {
        const ret = await esbuild.transform(code, {
          platform: 'node',
          loader: ext === '.tsx' ? 'tsx' : 'ts',
          format: 'esm',
          target: 'chrome80',
          sourcemap: true,
        });
        // console.log(
        //   'esbuild transform time:',
        //   performance.now() - now,
        //   nodePath.relative(process.cwd(), id),
        // )
        return {
          code: ret.code,
        };
      }
    },
  };
};

export default defineParallelPluginImplementation((_options, _context) => {
  return plugin();
});
