import type { BindingModuleInfo, BindingPluginContext } from '../binding.cjs';
import type { JsModuleOptions } from '../plugin/plugin-context-data';
import type { ModuleInfo } from '../types/module-info';
import { unsupported } from './misc';

/** The binding calls behind `ModuleInfo.moduleSideEffects`. Only Rust keeps the value. */
export type ModuleSideEffectsAccess = Pick<
  BindingPluginContext,
  'getModuleSideEffects' | 'setModuleSideEffects'
>;

/** `treeshake.moduleSideEffects` when it is a function. */
export type ModuleSideEffectsOptionFn = (id: string, external: boolean) => boolean | undefined;

export function transformModuleInfo(
  info: BindingModuleInfo,
  option: JsModuleOptions,
  access: ModuleSideEffectsAccess,
  callOption: ModuleSideEffectsOptionFn | undefined,
): ModuleInfo {
  const id = info.id;
  return {
    get ast() {
      return unsupported('ModuleInfo#ast');
    },
    get code() {
      return info.code;
    },
    id,
    importers: info.importers,
    dynamicImporters: info.dynamicImporters,
    importedIds: info.importedIds,
    dynamicallyImportedIds: info.dynamicallyImportedIds,
    exports: info.exports,
    isEntry: info.isEntry,
    inputFormat: info.inputFormat,
    meta: option.meta,
    // Each read asks Rust, so an info object that a plugin keeps stays current.
    get moduleSideEffects() {
      return access.getModuleSideEffects(id, callOption);
    },
    set moduleSideEffects(value: ModuleInfo['moduleSideEffects']) {
      access.setModuleSideEffects(id, value, callOption);
    },
  };
}
