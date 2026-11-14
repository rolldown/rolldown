import { getCodeFrame } from '../utils/code-frame';
import { locate } from './locate-character';
import type { RolldownLog } from './logging';

const INVALID_LOG_POSITION = 'INVALID_LOG_POSITION',
  PLUGIN_ERROR = 'PLUGIN_ERROR',
  INPUT_HOOK_IN_OUTPUT_PLUGIN = 'INPUT_HOOK_IN_OUTPUT_PLUGIN',
  CYCLE_LOADING = 'CYCLE_LOADING',
  MISSING_CODE_SPLITTING_GROUP_DEBUG_NAME = 'MISSING_CODE_SPLITTING_GROUP_DEBUG_NAME',
  PARSE_ERROR = 'PARSE_ERROR',
  NO_FS_IN_BROWSER = 'NO_FS_IN_BROWSER',
  PERSISTENT_CACHE_MODULE_INFO = 'PERSISTENT_CACHE_MODULE_INFO',
  PERSISTENT_CACHE_UNSAVABLE_META = 'PERSISTENT_CACHE_UNSAVABLE_META',
  VALIDATION_ERROR = 'VALIDATION_ERROR';

export function logParseError(message: string, id: string | undefined, pos?: number): RolldownLog {
  return {
    code: PARSE_ERROR,
    id,
    message,
    pos,
  };
}

export function logFailedValidation(message: string): RolldownLog {
  return {
    code: VALIDATION_ERROR,
    message,
  };
}

export function logInvalidLogPosition(pluginName: string): RolldownLog {
  return {
    code: INVALID_LOG_POSITION,
    message: `Plugin "${pluginName}" tried to add a file position to a log or warning. This is only supported in the "transform" hook at the moment and will be ignored.`,
  };
}

export function logInputHookInOutputPlugin(pluginName: string, hookName: string): RolldownLog {
  return {
    code: INPUT_HOOK_IN_OUTPUT_PLUGIN,
    message: `The "${hookName}" hook used by the output plugin ${pluginName} is a build time hook and will not be run for that plugin. Either this plugin cannot be used as an output plugin, or it should have an option to configure it as an output plugin.`,
  };
}

export function logCycleLoading(pluginName: string, moduleId: string): RolldownLog {
  return {
    code: CYCLE_LOADING,
    message: `Found the module "${moduleId}" cycle loading at ${pluginName} plugin, it maybe blocking fetching modules.`,
  };
}

export function logMissingCodeSplittingGroupDebugName(groupPath: string): RolldownLog {
  return {
    code: MISSING_CODE_SPLITTING_GROUP_DEBUG_NAME,
    message: `\`${groupPath}.name\` is a function. Set \`${groupPath}.debugName\` so the bundler timing report can identify this group.`,
  };
}

export function logPersistentCacheUnsavableMeta(
  pluginName: string,
  hookName: string,
  moduleId: string,
): RolldownLog {
  return {
    code: PERSISTENT_CACHE_UNSAVABLE_META,
    plugin: pluginName,
    hook: hookName,
    id: moduleId,
    message:
      `The "${hookName}" hook of the plugin "${pluginName}" returned a \`meta\` value that the persistent cache cannot save. ` +
      `The first module with this problem is "${moduleId}". Rolldown saves no result for such a module, so its hooks run again in every build. ` +
      `The cache saves \`meta\` as JSON. Use only strings, finite numbers, booleans, \`null\`, arrays and plain objects. ` +
      `For example, use an array in place of a \`Set\`.`,
  };
}

/**
 * How a hook call used a `meta` or `moduleSideEffects` value that the persistent cache does not
 * save with the hook result. The plugin can write the hook in another way, so Rolldown warns:
 * - `meta`: the hook read or changed `meta` through `this.getModuleInfo` or a `this.resolve`
 *   result, or returned the `meta` object of another module.
 * - `moduleSideEffectsRead`: the hook read `moduleSideEffects` through `this.getModuleInfo` or a
 *   `this.resolve` result.
 * - `moduleSideEffectsAssigned`: the hook assigned `moduleSideEffects` on a module info.
 */
export type PersistentCacheModuleInfoUse =
  | 'meta'
  | 'moduleSideEffectsRead'
  | 'moduleSideEffectsAssigned';

/**
 * `moduleId` is the module whose task gets no saved result. `target` is the module whose `meta`
 * or `moduleSideEffects` the hook used. They are the same module when the hook used its own.
 */
export function logPersistentCacheModuleInfo(
  pluginName: string,
  hookName: string,
  moduleId: string,
  target: string,
  use: PersistentCacheModuleInfoUse,
): RolldownLog {
  const hook = `The "${hookName}" hook of the plugin "${pluginName}"`;
  const noSavedResult =
    `So the hook must run in every build, and "${moduleId}" gets no saved result. ` +
    `Rolldown prints this warning once for each plugin and hook, so other modules can have the same problem.`;
  const message = {
    meta:
      `${hook} read or changed the \`meta\` of "${target}" through \`this.getModuleInfo()\` or a \`this.resolve()\` result. ` +
      `The persistent cache saves only the \`meta\` that a hook returns. It does not save a change made this way, and it cannot see when another hook changes a value that this hook read. ` +
      `${noSavedResult} To keep the saved result, return the \`meta\` from this hook. A spread such as \`{ ...resolved }\` also reads \`meta\`.`,
    moduleSideEffectsRead:
      `${hook} read the \`moduleSideEffects\` of "${target}" through \`this.getModuleInfo()\` or a \`this.resolve()\` result. ` +
      `Another hook can set this value, and the persistent cache cannot see when that hook changes it. ` +
      `${noSavedResult} To keep the saved result, do not read this value in a hook with the cache on. A spread such as \`{ ...resolved }\` also reads it.`,
    moduleSideEffectsAssigned:
      `${hook} assigned the \`moduleSideEffects\` of "${target}" through \`this.getModuleInfo()\`. ` +
      `The persistent cache saves only the \`moduleSideEffects\` that a hook returns, so it does not save this assignment. ` +
      `${noSavedResult} To keep the saved result, return \`moduleSideEffects\` from a hook instead: from the \`load\` or \`transform\` hook of "${target}", or from the \`resolveId\` hook that resolves "${target}".`,
  }[use];
  return {
    code: PERSISTENT_CACHE_MODULE_INFO,
    plugin: pluginName,
    hook: hookName,
    id: moduleId,
    message,
  };
}

export function logNoFileSystemInBrowser(method: string): RolldownLog {
  return {
    code: NO_FS_IN_BROWSER,
    message: `Cannot access the file system (via "${method}") when using the browser build of Rolldown.`,
  };
}

export function logPluginError(
  error: Omit<RolldownLog, 'code'> & { code?: unknown },
  plugin: string,
  { hook, id }: { hook?: string; id?: string } = {},
): RolldownLog {
  try {
    const code = error.code;
    if (
      !error.pluginCode &&
      code != null &&
      (typeof code !== 'string' || !code.startsWith('PLUGIN_'))
    ) {
      error.pluginCode = code;
    }
    error.code = PLUGIN_ERROR;
    error.plugin = plugin;
    if (hook) {
      error.hook = hook;
    }
    if (id) {
      error.id = id;
    }
    // eslint-disable-next-line no-unused-vars
  } catch (_) {
    // Ignore error, maybe the error can't be assigned.
  } finally {
    // eslint-disable-next-line no-unsafe-finally
    return error as RolldownLog;
  }
}

export function error(base: Error | RolldownLog): never {
  if (!(base instanceof Error)) {
    base = Object.assign(new Error(base.message), base);
    Object.defineProperty(base, 'name', {
      value: 'RolldownError',
      writable: true,
    });
  }
  throw base;
}

export function augmentCodeLocation(
  properties: RolldownLog,
  pos: number | { column: number; line: number },
  source: string,
  id: string,
): void {
  if (typeof pos === 'object') {
    const { line, column } = pos;
    properties.loc = { column, file: id, line };
  } else {
    properties.pos = pos;
    const location = locate(source, pos, { offsetLine: 1 });
    if (!location) {
      return;
    }
    const { line, column } = location;
    properties.loc = { column, file: id, line };
  }

  if (properties.frame === undefined) {
    const { line, column } = properties.loc;
    properties.frame = getCodeFrame(source, line, column);
  }
}
