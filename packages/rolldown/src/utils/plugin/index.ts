import {
  ENUMERATED_PLUGIN_HOOK_NAMES,
  PluginHookNames,
} from '../../constants/plugin'

export const isPluginHookName = (function () {
  const PLUGIN_HOOK_NAMES_SET = new Set(
    ENUMERATED_PLUGIN_HOOK_NAMES as readonly string[],
  )
  return function isPluginHookName(
    hookName: string,
  ): hookName is PluginHookNames {
    return PLUGIN_HOOK_NAMES_SET.has(hookName)
  }
})()
