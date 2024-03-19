import { readFileSync } from 'node:fs'
import path from 'node:path'
import { ERR_CLI_META_DATA } from './errors.js'

/**
 * get package.json
 *
 * @description Get the package.json of the target package
 *
 * @param {string} target - A target of the path of `package.json`
 * @returns {{ version: string, description: string }}
 */
export function getPackageJSON(target) {
  const raw = readFileSync(path.join(target, 'package.json'), 'utf8')
  const pkg = JSON.parse(raw)
  if (!pkg.name || !pkg.version || !pkg.description) {
    throw new Error(ERR_CLI_META_DATA)
  }
  return pkg
}
