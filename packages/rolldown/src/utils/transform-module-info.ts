import type { BindingModuleInfo, BindingPluginContext } from '../binding.cjs';
import type { JsModuleOptions } from '../plugin/plugin-context-data';
import type { ModuleInfo } from '../types/module-info';
import { unsupported } from './misc';

/**
 * The binding calls behind `ModuleInfo.moduleSideEffects`. Only Rust keeps the value.
 * `setModuleSideEffects` ignores a write after `buildEnd`, and returns `true` for the first one
 * in a build.
 */
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
  warnWriteAfterBuildEnd: (moduleId: string) => void,
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
      // Tree shaking has used the values after `buildEnd`, so Rust ignores a later write.
      if (access.setModuleSideEffects(id, value, callOption)) {
        warnWriteAfterBuildEnd(id);
      }
    },
  };
}

/**
 * Like {@linkcode transformModuleInfo}, but copies `code` — the only
 * box-backed field, and by far the largest retention — to plain JS data and
 * releases the native box immediately, for the threadless-WASI flavor. A
 * callback that retains the returned info past its hook keeps working.
 */
export function snapshotModuleInfo(
  info: BindingModuleInfo,
  option: JsModuleOptions,
  access: ModuleSideEffectsAccess,
  callOption: ModuleSideEffectsOptionFn | undefined,
  warnWriteAfterBuildEnd: (moduleId: string) => void,
): ModuleInfo {
  const id = info.id;
  const code = info.code;
  const snapshot: ModuleInfo = {
    get ast() {
      return unsupported('ModuleInfo#ast');
    },
    code,
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
      // Tree shaking has used the values after `buildEnd`, so Rust ignores a later write.
      if (access.setModuleSideEffects(id, value, callOption)) {
        warnWriteAfterBuildEnd(id);
      }
    },
  };
  info.dropInner();
  return snapshot;
}
