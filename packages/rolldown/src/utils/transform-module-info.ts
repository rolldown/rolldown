import type { ModuleInfo } from '../types/module-info'
import type { BindingModuleInfo } from '../binding'
import { unsupported } from './misc'
import { ModuleOptions } from '..'

export function transformModuleInfo(
  info: BindingModuleInfo,
  option: ModuleOptions,
): ModuleInfo {
  return {
    get ast() {
      return unsupported('ModuleInfo#ast')
    },
    get code() {
      return info.code
    },
    id: info.id,
    importers: info.importers,
    dynamicImporters: info.dynamicImporters,
    importedIds: info.importedIds,
    dynamicallyImportedIds: info.dynamicallyImportedIds,
    exports: info.exports,
    isEntry: info.isEntry,
    ...option,
  }
}
