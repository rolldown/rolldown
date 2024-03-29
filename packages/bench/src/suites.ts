import nodePath from 'node:path'
import { PROJECT_ROOT, REPO_ROOT } from './utils.js'

export interface BenchSuite {
  title: string
  inputs: string[]
  benchIterationForRollup?: number
}

export const suitesForCI: BenchSuite[] = [
  {
    title: 'threejs',
    inputs: [nodePath.join(REPO_ROOT, './temp/three/entry.js')],
  },
  {
    title: 'threejs10x',
    inputs: [nodePath.join(REPO_ROOT, './temp/three10x/entry.js')],
    benchIterationForRollup: 2,
  },
]

/**
 * @type {BenchSuite[]}
 */
export const suites = [
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
