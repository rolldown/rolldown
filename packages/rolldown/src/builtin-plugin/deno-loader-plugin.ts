import { BuiltinPlugin } from './constructors'

type DenoLoaderPluginConfig = {
  import_map_string: string
}

export function denoLoaderPlugin(
  config: DenoLoaderPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:deno-loader', config)
}
