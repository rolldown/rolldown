import { readFileSync } from 'node:fs'
import path from 'node:path'

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
    throw new Error('cli meta data error')
  }
  return pkg
}
