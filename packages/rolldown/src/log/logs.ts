import { getCodeFrame } from '../utils/code-frame';
import { locate } from './locate-character';
import type { RolldownLog } from './logging';

const INVALID_LOG_POSITION = 'INVALID_LOG_POSITION',
  PLUGIN_ERROR = 'PLUGIN_ERROR',
  INPUT_HOOK_IN_OUTPUT_PLUGIN = 'INPUT_HOOK_IN_OUTPUT_PLUGIN',
  CYCLE_LOADING = 'CYCLE_LOADING',
  MULTIPLE_WATCHER_OPTION = 'MULTIPLE_WATCHER_OPTION',
  PARSE_ERROR = 'PARSE_ERROR',
  NO_FS_IN_BROWSER = 'NO_FS_IN_BROWSER';

export function logParseError(message: string, id: string | undefined, pos?: number): RolldownLog {
  return {
    code: PARSE_ERROR,
    id,
    message,
    pos,
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

export function logMultipleWatcherOption(): RolldownLog {
  return {
    code: MULTIPLE_WATCHER_OPTION,
    message: `Found multiple watcher options at watch options, using first one to start watcher.`,
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
