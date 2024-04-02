import * as nodePath from 'node:path'
import * as nodeUrl from 'node:url'
import * as selfExports from './constants.js'

const __filename = nodeUrl.fileURLToPath(import.meta.url)
const __dirname = nodePath.dirname(__filename)

export const REPO_ROOT = nodePath.join(__dirname, '../..').normalize()

if (process.argv[1] === __filename) {
  // If this file is executed directly, print the exports
  console.log(selfExports)
}
