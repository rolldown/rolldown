import * as path from 'path-browserify'
import { AssetItem, FileItem } from '../../../wasm'

export type ModuleInfo = {
  title: string
  code: string
  autofocus?: boolean
}

export function normalizeModules(modules: ModuleInfo[]): FileItem[] {
  return modules.map(normalizeModule)
}

export function convertAssetListToModuleList(
  assetList: AssetItem[],
): ModuleInfo[] {
  return assetList.map((item) => {
    return {
      title: item.name,
      code: item.content,
    }
  })
}

/**
 * convert relative path into absolute path in memory fs
 *
 * */
function normalizeModule(module: ModuleInfo): FileItem {
  let title = module.title
  let code = module.code
  let isAbsolute = path.isAbsolute(title)
  if (!isAbsolute) {
    title = path.join('/', title)
  }
  return new FileItem(title, code)
}

let moduleId = 1

export function uniqueModulePath(modules: ModuleInfo[]): string {
  let curName = `module_${moduleId}.js`
  while (true) {
    let m = modules.find((item) => item.title === curName)
    if (!m) {
      break
    }
    curName = `module_${++moduleId}.js`
  }
  return curName
}
