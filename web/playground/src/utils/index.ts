import { isAbsolute, join } from 'pathe'
// @ts-expect-error
import { AssetItem, FileItem } from '@rolldown/wasm-binding'

export type ModuleInfo = {
  title: string
  code: string
  autofocus?: boolean
  isEntry: boolean
  canModifyEntry?: boolean
}

export function normalizeModules(modules: ModuleInfo[]): FileItem[] {
  return modules.map(normalizeModule)
}

// Only used when generate output
export function convertAssetListToModuleList(
  assetList: AssetItem[],
): ModuleInfo[] {
  return assetList.map((item) => {
    return {
      title: item.name,
      code: item.content,
      isEntry: false,
      canModifyEntry: false,
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
  let isEntry = module.isEntry
  let absolute = isAbsolute(title)
  if (!absolute) {
    title = join('/', title)
  }
  return new FileItem(title, code, isEntry)
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
