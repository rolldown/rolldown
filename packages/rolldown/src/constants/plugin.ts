import { Plugin } from '../plugin'

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
 * Names of all properties in a `Plugin` object. Includes `name` and `api`.
 */
export type PluginProps = keyof Plugin

type EnumeratedPluginHookNames = typeof ENUMERATED_PLUGIN_HOOK_NAMES
/**
 * Names of all hooks in a `Plugin` object. Does not include `name` and `api`, since they are not hooks.
 */
export type PluginHookNames = EnumeratedPluginHookNames[number]

/**
 * Names of all defined hooks. It's like
 * ```ts
 * type DefinedHookNames = {
 *   options: 'options',
 *   buildStart: 'buildStart',
 *   ...
 * }
 * ```
 */
export type DefinedHookNames = {
  readonly [K in PluginHookNames]: K
}

/**
 * Names of all defined hooks. It's like
 * ```js
 * const DEFINED_HOOK_NAMES ={
 *   options: 'options',
 *   buildStart: 'buildStart',
 *   ...
 * }
 * ```
 */
export const DEFINED_HOOK_NAMES: DefinedHookNames = {
  [ENUMERATED_PLUGIN_HOOK_NAMES[0]]: ENUMERATED_PLUGIN_HOOK_NAMES[0],
  [ENUMERATED_PLUGIN_HOOK_NAMES[1]]: ENUMERATED_PLUGIN_HOOK_NAMES[1],
  [ENUMERATED_PLUGIN_HOOK_NAMES[2]]: ENUMERATED_PLUGIN_HOOK_NAMES[2],
  [ENUMERATED_PLUGIN_HOOK_NAMES[3]]: ENUMERATED_PLUGIN_HOOK_NAMES[3],
  [ENUMERATED_PLUGIN_HOOK_NAMES[4]]: ENUMERATED_PLUGIN_HOOK_NAMES[4],
  [ENUMERATED_PLUGIN_HOOK_NAMES[5]]: ENUMERATED_PLUGIN_HOOK_NAMES[5],
  [ENUMERATED_PLUGIN_HOOK_NAMES[6]]: ENUMERATED_PLUGIN_HOOK_NAMES[6],
  [ENUMERATED_PLUGIN_HOOK_NAMES[7]]: ENUMERATED_PLUGIN_HOOK_NAMES[7],
  [ENUMERATED_PLUGIN_HOOK_NAMES[8]]: ENUMERATED_PLUGIN_HOOK_NAMES[8],
  [ENUMERATED_PLUGIN_HOOK_NAMES[9]]: ENUMERATED_PLUGIN_HOOK_NAMES[9],
  [ENUMERATED_PLUGIN_HOOK_NAMES[10]]: ENUMERATED_PLUGIN_HOOK_NAMES[10],
  [ENUMERATED_PLUGIN_HOOK_NAMES[11]]: ENUMERATED_PLUGIN_HOOK_NAMES[11],
  [ENUMERATED_PLUGIN_HOOK_NAMES[12]]: ENUMERATED_PLUGIN_HOOK_NAMES[12],
  [ENUMERATED_PLUGIN_HOOK_NAMES[13]]: ENUMERATED_PLUGIN_HOOK_NAMES[13],
  [ENUMERATED_PLUGIN_HOOK_NAMES[14]]: ENUMERATED_PLUGIN_HOOK_NAMES[14],
  [ENUMERATED_PLUGIN_HOOK_NAMES[15]]: ENUMERATED_PLUGIN_HOOK_NAMES[15],
  [ENUMERATED_PLUGIN_HOOK_NAMES[16]]: ENUMERATED_PLUGIN_HOOK_NAMES[16],
  [ENUMERATED_PLUGIN_HOOK_NAMES[17]]: ENUMERATED_PLUGIN_HOOK_NAMES[17],
  [ENUMERATED_PLUGIN_HOOK_NAMES[18]]: ENUMERATED_PLUGIN_HOOK_NAMES[18],
  [ENUMERATED_PLUGIN_HOOK_NAMES[19]]: ENUMERATED_PLUGIN_HOOK_NAMES[19],
} as const
