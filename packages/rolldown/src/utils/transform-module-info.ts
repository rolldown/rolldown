import type { ModuleOptions } from '..';
import type { BindingModuleInfo } from '../binding.cjs';
import type { ModuleInfo } from '../types/module-info';
import { unsupported } from './misc';

type ReportedModuleSideEffects = Exclude<ModuleOptions['moduleSideEffects'], undefined>;
type AssignedModuleSideEffects = ReportedModuleSideEffects | undefined;

/**
 * A hook or a direct write stores `moduleSideEffects` on `option`. `undefined` means
 * nothing was stored, so the native effective value (hook, `treeshake.moduleSideEffects`,
 * `package.json` `sideEffects`, then `true`, including externals and `treeshake: false`)
 * is used. An explicit `null` write stays `null`.
 */
export function effectiveModuleSideEffects(
  assigned: AssignedModuleSideEffects,
  fallback: ReportedModuleSideEffects | null | undefined,
): ReportedModuleSideEffects {
  return assigned === undefined ? (fallback ?? null) : assigned;
}

export function transformModuleInfo(
  info: BindingModuleInfo,
  option: Omit<ModuleOptions, 'moduleSideEffects'> & {
    moduleSideEffects: AssignedModuleSideEffects;
  },
): ModuleInfo {
  const { moduleSideEffects, ...rest } = option;
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
    ...rest,
    moduleSideEffects: effectiveModuleSideEffects(moduleSideEffects, info.moduleSideEffects),
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
  option: Omit<ModuleOptions, 'moduleSideEffects'> & {
    moduleSideEffects: AssignedModuleSideEffects;
  },
): ModuleInfo {
  const { moduleSideEffects, ...rest } = option;
  const code = info.code;
  const nativeModuleSideEffects = info.moduleSideEffects;
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
    ...rest,
    moduleSideEffects: effectiveModuleSideEffects(moduleSideEffects, nativeModuleSideEffects),
  };
  info.dropInner();
  return snapshot;
}
