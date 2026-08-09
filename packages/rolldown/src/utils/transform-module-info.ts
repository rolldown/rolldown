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
 * Like {@linkcode transformModuleInfo}, but copies `code` (the one field that
 * reads through the native box, and by far the largest retention: the module's
 * full source) to plain JS data and releases the native `BindingModuleInfo`
 * box immediately. For the threadless-WASI flavor, where GC finalizers (the
 * box's normal reclamation path) never run. A callback that retains the
 * returned info past its hook invocation keeps working: nothing on it reads
 * through the released box.
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
