import { BuiltinPlugin } from './constructors'

type DenoLoaderPluginConfig = {
  importMap: string
  importMapBaseUrl: string
}

export function denoLoaderPlugin(
  config: DenoLoaderPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:deno-loader', config)
}
