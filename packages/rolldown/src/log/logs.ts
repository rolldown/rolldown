import type { RollupLog } from '../types/misc';
import { getCodeFrame } from '../utils/code-frame';
import { locate } from './locate-character';

const INVALID_LOG_POSITION = 'INVALID_LOG_POSITION',
  PLUGIN_ERROR = 'PLUGIN_ERROR',
  INPUT_HOOK_IN_OUTPUT_PLUGIN = 'INPUT_HOOK_IN_OUTPUT_PLUGIN',
  CYCLE_LOADING = 'CYCLE_LOADING',
  MULTIPLY_NOTIFY_OPTION = 'MULTIPLY_NOTIFY_OPTION',
  PARSE_ERROR = 'PARSE_ERROR';

export function logParseError(message: string): RollupLog {
  return {
    code: PARSE_ERROR,
    message,
  };
}

export function logInvalidLogPosition(pluginName: string): RollupLog {
  return {
    code: INVALID_LOG_POSITION,
    message:
      `Plugin "${pluginName}" tried to add a file position to a log or warning. This is only supported in the "transform" hook at the moment and will be ignored.`,
  };
}

export function logInputHookInOutputPlugin(
  pluginName: string,
  hookName: string,
): RollupLog {
  return {
    code: INPUT_HOOK_IN_OUTPUT_PLUGIN,
    message:
      `The "${hookName}" hook used by the output plugin ${pluginName} is a build time hook and will not be run for that plugin. Either this plugin cannot be used as an output plugin, or it should have an option to configure it as an output plugin.`,
  };
}

export function logCycleLoading(
  pluginName: string,
  moduleId: string,
): RollupLog {
  return {
    code: CYCLE_LOADING,
    message:
      `Found the module "${moduleId}" cycle loading at ${pluginName} plugin, it maybe blocking fetching modules.`,
  };
}

export function logMultiplyNotifyOption(): RollupLog {
  return {
    code: MULTIPLY_NOTIFY_OPTION,
    message:
      `Found multiply notify option at watch options, using first one to start notify watcher.`,
  };
}

export function logPluginError(
  error: Omit<RollupLog, 'code'> & { code?: unknown },
  plugin: string,
  { hook, id }: { hook?: string; id?: string } = {},
) {
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
  return error as RollupLog;
}

export function error(base: Error | RollupLog): never {
  if (!(base instanceof Error)) {
    base = Object.assign(new Error(base.message), base);
    Object.defineProperty(base, 'name', {
      value: 'RollupError',
      writable: true,
    });
  }
  throw base;
}

export function augmentCodeLocation(
  properties: RollupLog,
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
