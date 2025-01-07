import { readFileSync } from 'node:fs'
import path from 'node:path'

const nodeVersion = readFileSync(
  path.join(__dirname, '../../.node-version'),
  'utf8',
).trim()

export default {
  load() {
    return {
      nodeVersion,
    }
  },
}
