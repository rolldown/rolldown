import * as selfExports from './constants.js'
import { workspaceRoot } from '@rolldown/testing'

export const REPO_ROOT = workspaceRoot()

if (process.argv[1] === __filename) {
  // If this file is executed directly, print the exports
  console.log(selfExports)
}
