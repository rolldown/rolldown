import type { ModuleOptions } from '..';
import type { BindingModuleInfo } from '../binding.cjs';
import type { ModuleInfo } from '../types/module-info';
import { unsupported } from './misc';

export function transformModuleInfo(info: BindingModuleInfo, option: ModuleOptions): ModuleInfo {
  // Ensure meta.commonjs.isCommonJS is set for backward compatibility with Rollup
  const meta = option.meta || {};
  if (!meta.commonjs) {
    meta.commonjs = {};
  }
  meta.commonjs.isCommonJS = info.isCommonjs;

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
    isCommonJS: info.isCommonjs,
    ...option,
    meta,
  };
}
