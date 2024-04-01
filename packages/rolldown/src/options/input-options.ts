import {
  NormalizedInputOptions,
  InputOptions as RollupInputOptions,
} from '../rollup-types'
import { ensureArray, normalizePluginOption } from '../utils'
import { BindingResolveOptions } from '../binding'
import { Plugin } from '../plugin'

// TODO export compat plugin type
export interface InputOptions {
  input?: RollupInputOptions['input']
  plugins?: Plugin[]
  external?: RollupInputOptions['external']
  resolve?: RolldownResolveOptions
  cwd?: string
}

export type RolldownResolveOptions = Omit<BindingResolveOptions, 'alias'> & {
  alias?: Record<string, string>
}

export type RolldownNormalizedInputOptions = NormalizedInputOptions & {
  resolve?: BindingResolveOptions
}

export async function normalizeInputOptions(
  config: InputOptions,
): Promise<RolldownNormalizedInputOptions> {
  // @ts-expect-error
  return {
    input: getInput(config),
    plugins: await normalizePluginOption(config.plugins),
    external: getIdMatcher(config.external),
    resolve: getResolve(config.resolve),
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

function getResolve(
  resolve?: RolldownResolveOptions,
): RolldownNormalizedInputOptions['resolve'] {
  if (resolve) {
    return {
      ...resolve,
      alias: resolve.alias
        ? Object.fromEntries(
            Object.entries(resolve.alias).map(([key, value]) => [key, [value]]),
          )
        : undefined,
    }
  }
}
