import nodePath from 'node:path'
import { PROJECT_ROOT, REPO_ROOT } from './utils.js'

/**
 * @type {import('./types.ts').BenchSuite[]}
 */
export const suitesForCI = [
  {
    title: 'threejs10x',
    inputs: [nodePath.join(REPO_ROOT, './tmp/bench/three10x/entry.js')],
    disableRollup: true,
  },
]

/**
 * @type {import('./types.js').BenchSuite[]}
 */
export const suites = [
  {
    title: 'threejs',
    inputs: [nodePath.join(REPO_ROOT, './tmp/bench/three/entry.js')],
  },
  {
    title: 'vue-stack',
    inputs: [nodePath.join(PROJECT_ROOT, 'vue-entry.js')],
  },
  {
    title: 'react-stack',
    inputs: ['react', 'react-dom'],
  },
  ...suitesForCI,
]
