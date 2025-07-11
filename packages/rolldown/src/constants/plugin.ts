export const ENUMERATED_INPUT_PLUGIN_HOOK_NAMES = [
  'options',
  'buildStart',
  'resolveId',
  'load',
  'transform',
  'moduleParsed',
  'buildEnd',
  'onLog',
  'resolveDynamicImport',
  'closeBundle',
  'closeWatcher',
  'watchChange',
] as const;

const ENUMERATED_OUTPUT_PLUGIN_HOOK_NAMES = [
  'augmentChunkHash',
  'outputOptions',
  'renderChunk',
  'renderStart',
  'renderError',
  'writeBundle',
  'generateBundle',
] as const;

const ENUMERATED_PLUGIN_HOOK_NAMES: [
  ...typeof ENUMERATED_INPUT_PLUGIN_HOOK_NAMES,
  ...typeof ENUMERATED_OUTPUT_PLUGIN_HOOK_NAMES,
  'footer',
  'banner',
  'intro',
  'outro',
] = [
  // build hooks
  ...ENUMERATED_INPUT_PLUGIN_HOOK_NAMES,
  // generate hooks
  ...ENUMERATED_OUTPUT_PLUGIN_HOOK_NAMES,
  // addon hooks
  'footer',
  'banner',
  'intro',
  'outro',
] as const;

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
  readonly [K in typeof ENUMERATED_PLUGIN_HOOK_NAMES[number]]: K;
};

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
  [ENUMERATED_PLUGIN_HOOK_NAMES[20]]: ENUMERATED_PLUGIN_HOOK_NAMES[20],
  [ENUMERATED_PLUGIN_HOOK_NAMES[21]]: ENUMERATED_PLUGIN_HOOK_NAMES[21],
  [ENUMERATED_PLUGIN_HOOK_NAMES[22]]: ENUMERATED_PLUGIN_HOOK_NAMES[22],
} as const;
