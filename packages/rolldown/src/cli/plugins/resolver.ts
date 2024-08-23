// Adapted from rollup/cli/run/commandPlugins.ts
import { RolldownPlugin } from '../../plugin'
import { pathToFileURL } from 'node:url'
import type { BuiltinPlugin } from '../../plugin/builtin-plugin'
import * as builtinPlugins from '../../plugin/builtin-plugin'
import { resolve } from 'node:path'

export async function resolveCommandPlugin(
  plugin: string,
): Promise<RolldownPlugin> {
  const builtinResolved = await resolveAsBuiltinPlugin(plugin)
  if (builtinResolved) {
    return builtinResolved
  }
  return await loadAndRegisterCustomizedPlugin(plugin)
}

async function loadAndRegisterCustomizedPlugin(
  text: string,
): Promise<RolldownPlugin> {
  let plugin: any = undefined
  let pluginArgument: any = undefined
  if (text[0] === '{') {
    // -p "{transform(c,i){...}}"
    plugin = new Function('return ' + text)
  } else {
    const match = text.match(/^([\w./:@\\^{|}-]+)(=(.*))?$/)
    if (match) {
      // -p plugin
      // -p plugin=arg
      text = match[1]
      pluginArgument = new Function('return ' + match[3])()
    } else {
      throw new Error(
        `Invalid --plugin argument format: ${JSON.stringify(text)}`,
      )
    }
    if (!/^\.|^(rollup|rolldown)-plugin-|[/@\\]/.test(text)) {
      // Try using plugin prefix variations first if applicable.
      // Prefix order is significant - left has higher precedence.
      for (const prefix of [
        '@rollup/plugin-',
        'rollup-plugin-',
        '@rolldown/plugin-',
        'rolldown-plugin-',
      ]) {
        try {
          plugin = await requireOrImport(prefix + text)
          break
        } catch {
          // if this does not work, we try requiring the actual name below
        }
      }
    }
    if (!plugin) {
      try {
        if (text[0] == '.') {
          text = resolve(text)
        }
        // Windows absolute paths must be specified as file:// protocol URL
        // Note that we do not have coverage for Windows-only code paths
        else if (/^[A-Za-z]:\\/.test(text)) {
          text = pathToFileURL(resolve(text)).href
        }
        plugin = await requireOrImport(text)
      } catch (error: any) {
        throw new Error(`Cannot load plugin "${text}": ${error.message}.`)
      }
    }
  }
  // some plugins do not use `module.exports` for their entry point,
  // in which case we try the named default export and the plugin name
  if (typeof plugin === 'object') {
    plugin = plugin.default || plugin[getCamelizedPluginBaseName(text)]
  }
  if (!plugin) {
    throw new Error(
      `Cannot find entry for plugin "${text}". The plugin needs to export a function either as "default" or "${getCamelizedPluginBaseName(
        text,
      )}" for Rolldown to recognize it.`,
    )
  }
  return typeof plugin === 'function'
    ? plugin.call(plugin, pluginArgument)
    : plugin
}

export async function resolveAsBuiltinPlugin(
  name: string,
): Promise<BuiltinPlugin | undefined> {
  if (name.startsWith('builtin:')) {
    name = name.slice('builtin:'.length)
  }
  switch (name) {
    case 'module-preload-polyfill':
      builtinPlugins.modulePreloadPolyfillPlugin()
      break
    case 'dynamic-import-vars':
      builtinPlugins.dynamicImportVarsPlugin()
      break
    case 'import-glob':
      builtinPlugins.importGlobPlugin()
      break
    case 'manifest':
      builtinPlugins.manifestPlugin()
      break
    case 'wasm-helper':
      builtinPlugins.wasmHelperPlugin()
      break
    case 'wasm-fallback':
      builtinPlugins.wasmFallbackPlugin()
      break
    case 'transform':
      builtinPlugins.transformPlugin()
      break
    case 'load-fallback':
      builtinPlugins.loadFallbackPlugin()
      break
    case 'json':
      builtinPlugins.jsonPlugin()
      break
    default:
      // alias plugin doesn't support non-config options
      return
  }
}

function getCamelizedPluginBaseName(text: string): string {
  return (
    text.match(
      /(@rollup\/plugin-|rollup-plugin-|@rolldown\/plugin-|rolldown-plugin-)(.+)$/,
    )?.[2] || text
  )
    .split(/[/\\]/)
    .slice(-1)[0]
    .split('.')[0]
    .split('-')
    .map((part, index) =>
      index === 0 || !part ? part : part[0].toUpperCase() + part.slice(1),
    )
    .join('')
}

async function requireOrImport(pluginPath: string): Promise<any> {
  try {
    // eslint-disable-next-line unicorn/prefer-module
    return require(pluginPath)
  } catch {
    return import(pluginPath)
  }
}
