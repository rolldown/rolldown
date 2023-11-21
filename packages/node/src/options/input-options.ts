import {
  NormalizedInputOptions,
  InputOptions as RollupInputOptions,
  Plugin,
} from '../rollup-types'
import { ensureArray, normalizePluginOption } from '../utils'

// TODO export compat plugin type
export type RolldownPlugin = Plugin
export interface InputOptions {
  input?: RollupInputOptions['input']
  plugins?: RolldownPlugin[]
  external?: RollupInputOptions['external']
}

export async function normalizeInputOptions(
  config: InputOptions,
): Promise<NormalizedInputOptions> {
  // @ts-expect-error
  return {
    input: getInput(config),
    plugins: await normalizePluginOption(config.plugins),
    external: getIdMatcher(config.external),
  }
}

function getInput(config: InputOptions): NormalizedInputOptions['input'] {
  const configInput = config.input
  return configInput == null
    ? []
    : typeof configInput === 'string'
    ? [configInput]
    : configInput
}

const getIdMatcher = <T extends Array<any>>(
  option:
    | undefined
    // | boolean
    | string
    | RegExp
    | (string | RegExp)[]
    | ((id: string, ...parameters: T) => boolean | null | void),
): ((id: string, ...parameters: T) => boolean) => {
  // if (option === true) {
  // 	return () => true;
  // }
  if (typeof option === 'function') {
    return (id, ...parameters) =>
      (!id.startsWith('\0') && option(id, ...parameters)) || false
  }
  if (option) {
    const ids = new Set<string>()
    const matchers: RegExp[] = []
    for (const value of ensureArray(option)) {
      if (value instanceof RegExp) {
        matchers.push(value)
      } else {
        ids.add(value)
      }
    }
    return (id: string, ..._arguments) =>
      ids.has(id) || matchers.some((matcher) => matcher.test(id))
  }
  // Rollup here convert `undefined` to function, it is bad for performance. So it will convert to `undefined` at adapter.
  return () => false
}
