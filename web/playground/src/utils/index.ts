import * as path from 'path-browserify'
import { AssetItem, FileItem } from "../../../wasm";

export type ModuleInfo = {
	title: string;
	code: string
  autofucos?: boolean
};

export function normalizeModules(modules: ModuleInfo[]): FileItem[] {
  return modules.map(normalizeModule)
}

export function convertAssetListToModuleList(assetList: AssetItem[]): ModuleInfo[] {
  return assetList.map(item => {
    return {
      title: item.name,
      code: item.content
    }
  })
}


/**
 * convert relative path into absolute path in memory fs   
 *
 * */
function normalizeModule(module: ModuleInfo): FileItem {
  let isAbsolute = path.isAbsolute(module.title)
  if (!isAbsolute) {
    module.title = path.join("/", module.title)
  }
  return new FileItem(module.title, module.code)
}


let moduleId = 1;

export function uniqueModulePath(modules: ModuleInfo[]): string {
  let curName = `module_${moduleId}.js`
  while (true) {
    let m = modules.find(item => item.title === curName)
    if (!m) {
      break;
    }
    moduleId ++;
  }
  return curName
}
