import type { ModuleInfo } from '../types/module-info'
import type { BindingModuleInfo } from '../binding'
import { unsupported } from './misc'

export function transformModuleInfo(info: BindingModuleInfo): ModuleInfo {
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
    isEntry: info.isEntry,
  }
}
