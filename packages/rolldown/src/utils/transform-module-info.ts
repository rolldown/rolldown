import type { ModuleOptions } from '..';
import type { BindingModuleInfo } from '../binding.cjs';
import type { ModuleInfo } from '../types/module-info';
import { unsupported } from './misc';

export function transformModuleInfo(info: BindingModuleInfo, option: ModuleOptions): ModuleInfo {
  return {
    get ast() {
      return unsupported('ModuleInfo#ast');
    },
    get code() {
      return info.code;
    },
    id: info.id,
    importers: info.importers,
    dynamicImporters: info.dynamicImporters,
    importedIds: info.importedIds,
    dynamicallyImportedIds: info.dynamicallyImportedIds,
    exports: info.exports,
    isEntry: info.isEntry,
    inputFormat: info.inputFormat,
    ...option,
  };
}

/**
 * Like {@linkcode transformModuleInfo}, but copies `code` — the only
 * box-backed field, and by far the largest retention — to plain JS data and
 * releases the native box immediately, for the threadless-WASI flavor. A
 * callback that retains the returned info past its hook keeps working.
 */
export function snapshotModuleInfo(info: BindingModuleInfo, option: ModuleOptions): ModuleInfo {
  const code = info.code;
  const snapshot: ModuleInfo = {
    get ast() {
      return unsupported('ModuleInfo#ast');
    },
    code,
    id: info.id,
    importers: info.importers,
    dynamicImporters: info.dynamicImporters,
    importedIds: info.importedIds,
    dynamicallyImportedIds: info.dynamicallyImportedIds,
    exports: info.exports,
    isEntry: info.isEntry,
    inputFormat: info.inputFormat,
    ...option,
  };
  info.dropInner();
  return snapshot;
}
