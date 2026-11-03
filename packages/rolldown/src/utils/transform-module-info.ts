import type { ModuleOptions } from '..';
import type { BindingModuleInfo } from '../binding.cjs';
import type { ModuleInfo } from '../types/module-info';
import { unsupported } from './misc';

type ReportedModuleSideEffects = Exclude<ModuleOptions['moduleSideEffects'], undefined>;
type AssignedModuleSideEffects = ReportedModuleSideEffects | undefined;

/**
 * A hook or a direct write stores `moduleSideEffects` on `option`. `undefined` means
 * nothing was stored, so the native effective value (hook, `treeshake.moduleSideEffects`,
 * `package.json` `sideEffects`, then `true`) is used. An explicit `null` write stays `null`.
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
