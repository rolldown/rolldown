import babel from '@babel/core';
import nodePath from 'node:path';
import type { Plugin } from 'rolldown';
import {
  defineParallelPluginImplementation,
  type ParallelPluginImplementation,
} from 'rolldown/parallelPlugin';

export const babelPlugin = (): Plugin => {
  const partialConfig = babel.loadPartialConfig({
    presets: [
      ['@babel/preset-env', { bugfixes: true }],
      '@babel/preset-typescript',
    ],
    targets: 'chrome >= 80',
    sourceMaps: true,
    configFile: false,
    browserslistConfigFile: false,
  });

  return {
    name: 'parallel-babel-plugin',
    transform(code, id) {
      const ext = nodePath.extname(id);
      if (ext === '.ts' || ext === '.tsx') {
        let now = performance.now();
        const ast = babel.parseSync(code, {
          ...partialConfig?.options,
          filename: id,
        });
        if (!ast || !partialConfig || !partialConfig.options) {
          throw new Error('failed to parse');
        }
        let diffAst = performance.now() - now;
        const ret = /** @type {babel.BabelFileResult} */ babel
          .transformFromAstSync(
            ast,
            code,
            {
              ...partialConfig?.options,
              filename: id,
            },
          );
        let diffTrans = performance.now() - now - diffAst;
        console.log(
          id,
          'total',
          diffAst + diffTrans,
          'parse: ',
          diffAst,
          'trans: ',
          diffTrans,
        );
        return { code: /** @type {string} */ ret?.code ?? void 0 };
      }
    },
  };
};

const impl: ParallelPluginImplementation = defineParallelPluginImplementation(
  (_options, _context) => {
    return babelPlugin();
  },
);

/** @public referenced by ./index.ts */
export default impl;
