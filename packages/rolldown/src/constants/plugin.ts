import { Plugin } from '../plugin'

/**
 * Names of all properties in a `Plugin` object. Includes `name` and `api`.
 */
export type PluginProps = keyof Plugin

/**
 * Names of all hooks in a `Plugin` object. Does not include `name` and `api`, since they are not hooks.
 */
export type PluginHookNames = Exclude<PluginProps, 'name' | 'api'>

// TODO: we need to make sure the defined hooks is the same as the actual hooks
export const ENUMERATED_PLUGIN_HOOK_NAMES = [
  // build hooks
  'options',
  'buildStart',
  'resolveId',
  'load',
  'transform',
  'moduleParsed',
  'augmentChunkHash',
  'buildEnd',
  'onLog',
  'resolveDynamicImport',
  // generate hooks
  'generateBundle',
  'outputOptions',
  'renderChunk',
  'renderStart',
  'renderError',
  'writeBundle',
  'footer',
  'banner',
  'intro',
  'outro',
] as const

/**
 * Use `isPluginHookName` rather than `PLUGIN_HOOK_NAMES_SET.has` to have a type-friendly check for a plugin hook name.
 */
export const PLUGIN_HOOK_NAMES_SET = new Set(
  ENUMERATED_PLUGIN_HOOK_NAMES as readonly string[],
)
