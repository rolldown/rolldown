import { PLUGIN_HOOK_NAMES_SET, PluginHookNames } from '../../constants/plugin'

export function isPluginHookName(
  hookName: string,
): hookName is PluginHookNames {
  return PLUGIN_HOOK_NAMES_SET.has(hookName)
}
