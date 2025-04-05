import { cloneDeepWith, set } from 'lodash-es';
import nodePath from 'node:path';
import type { BenchSuite } from '../types.js';
import { PROJECT_ROOT, REPO_ROOT } from '../utils.js';
import { suiteRomeTs } from './rome-ts.js';

export const suitesForCI = [
  {
    title: 'threejs10x',
    inputs: [nodePath.join(REPO_ROOT, './tmp/bench/three10x/entry.js')],
    disableBundler: 'rollup',
    rolldownOptions: {
      logLevel: 'silent',
    },
    derived: {
      sourcemap: true,
      minify: true,
    },
  },
  suiteRomeTs,
];

/**
 * @type {import('../types.js').BenchSuite[]}
 */
export const suites = [
  {
    title: 'threejs',
    inputs: [nodePath.join(REPO_ROOT, './tmp/bench/three/entry.js')],
    rolldownOptions: {
      logLevel: 'silent',
    },
  },
  {
    title: 'vue-stack',
    inputs: [nodePath.join(PROJECT_ROOT, 'vue-entry.js')],
    derived: {
      sourcemap: true,
    },
  },
  {
    title: 'react-stack',
    inputs: ['react', 'react-dom'],
  },
  ...suitesForCI,
];

export function expandSuitesWithDerived(suites: BenchSuite[]) {
  return suites.flatMap((suite) => {
    const expanded = [suite];
    if (suite.derived?.sourcemap) {
      const derived = cloneDeepWith(suite, (value) => {
        // We should pay attention to this while using singletons in config
        if (typeof value === 'function') {
          return value;
        }
      });
      derived.title = `${suite.title}-sourcemap`;
      delete derived.derived;
      set(derived, 'esbuildOptions.sourcemap', true);
      set(derived, 'rolldownOptions.output.sourcemap', true);
      set(derived, 'rollupOptions.output.sourcemap', true);
      expanded.push(derived);
    }
    if (suite.derived?.minify) {
      const derived = cloneDeepWith(suite, (value) => {
        // We should pay attention to this while using singletons in config
        if (typeof value === 'function') {
          return value;
        }
      });
      derived.title = `${suite.title}-minify`;
      delete derived.derived;
      set(derived, 'esbuildOptions.minify', true);
      set(derived, 'rolldownOptions.output.minify', true);
      expanded.push(derived);
    }
    if (suite.derived?.minify && suite.derived?.sourcemap) {
      const derived = cloneDeepWith(suite, (value) => {
        // We should pay attention to this while using singletons in config
        if (typeof value === 'function') {
          return value;
        }
      });
      derived.title = `${suite.title}-minify-sourcemap`;
      delete derived.derived;
      set(derived, 'esbuildOptions.sourcemap', true);
      set(derived, 'rolldownOptions.output.sourcemap', true);
      set(derived, 'esbuildOptions.minify', true);
      set(derived, 'rolldownOptions.output.minify', true);
      expanded.push(derived);
    }
    return expanded;
  });
}
