import { normalizeHook } from '../utils/normalize-hook'
import type { BindingPluginOptions } from '../binding'
import type { NormalizedInputOptions } from '../options/normalized-input-options'
import type { FunctionPluginHooks, Plugin } from './index'
import {
  ChangedOutputs,
  collectChangedBundle,
  transformToOutputBundle,
} from '../utils/transform-to-rollup-output'
import { PluginContext } from './plugin-context'
import { bindingifySourcemap } from '../types/sourcemap'
import { NormalizedOutputOptions } from '../options/normalized-output-options'
import { PluginContextData } from './plugin-context-data'
import {
  PluginHookWithBindingExt,
  bindingifyPluginHookMeta,
} from './bindingify-plugin-hook-meta'
import { transformToRenderedModule } from '../utils/transform-rendered-module'
import { error, logPluginError } from '../log/logs'

export function bindingifyRenderStart(
  plugin: Plugin,
  options: NormalizedInputOptions,
  outputOptions: NormalizedOutputOptions,
  pluginContextData: PluginContextData,
): PluginHookWithBindingExt<BindingPluginOptions['renderStart']> {
  const hook = plugin.renderStart
  if (!hook) {
    return {}
  }
  const { handler, meta } = normalizeHook(hook)

  return {
    plugin: async (ctx) => {
      try {
        handler.call(
          new PluginContext(options, ctx, plugin, pluginContextData),
          outputOptions,
          options,
        )
      } catch (e: any) {
        return error(
          logPluginError(e, plugin.name || '<unknown>', {
            hook: 'renderStart',
          }),
        )
      }
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}

export function bindingifyRenderChunk(
  plugin: Plugin,
  options: NormalizedInputOptions,
  outputOptions: NormalizedOutputOptions,
  pluginContextData: PluginContextData,
): PluginHookWithBindingExt<BindingPluginOptions['renderChunk']> {
  const hook = plugin.renderChunk
  if (!hook) {
    return {}
  }
  const { handler, meta } = normalizeHook(hook)

  return {
    plugin: async (ctx, code, chunk) => {
      Object.entries(chunk.modules).forEach(([key, module]) => {
        chunk.modules[key] = transformToRenderedModule(module)
      })

      let ret: ReturnType<FunctionPluginHooks['renderChunk']>
      try {
        ret = await handler.call(
          new PluginContext(options, ctx, plugin, pluginContextData),
          code,
          chunk,
          outputOptions,
        )
      } catch (e: any) {
        return error(
          logPluginError(e, plugin.name || '<unknown>', {
            hook: 'renderChunk',
          }),
        )
      }

      if (ret == null) {
        return
      }

      if (typeof ret === 'string') {
        return { code: ret }
      }

      if (!ret.map) {
        return { code: ret.code }
      }

      return {
        code: ret.code,
        map: bindingifySourcemap(ret.map),
      }
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}

export function bindingifyAugmentChunkHash(
  plugin: Plugin,
  options: NormalizedInputOptions,
  pluginContextData: PluginContextData,
): PluginHookWithBindingExt<BindingPluginOptions['augmentChunkHash']> {
  const hook = plugin.augmentChunkHash
  if (!hook) {
    return {}
  }
  const { handler, meta } = normalizeHook(hook)

  return {
    plugin: async (ctx, chunk) => {
      Object.entries(chunk.modules).forEach(([key, module]) => {
        chunk.modules[key] = transformToRenderedModule(module)
      })

      try {
        return await handler.call(
          new PluginContext(options, ctx, plugin, pluginContextData),
          chunk,
        )
      } catch (e: any) {
        return error(
          logPluginError(e, plugin.name || '<unknown>', {
            hook: 'augmentChunkHash',
          }),
        )
      }
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}

export function bindingifyRenderError(
  plugin: Plugin,
  options: NormalizedInputOptions,
  pluginContextData: PluginContextData,
): PluginHookWithBindingExt<BindingPluginOptions['renderError']> {
  const hook = plugin.renderError
  if (!hook) {
    return {}
  }
  const { handler, meta } = normalizeHook(hook)

  return {
    plugin: async (ctx, err) => {
      try {
        await handler.call(
          new PluginContext(options, ctx, plugin, pluginContextData),
          new Error(err),
        )
      } catch (e: any) {
        return error(
          logPluginError(e, plugin.name || '<unknown>', {
            hook: 'renderError',
          }),
        )
      }
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}

export function bindingifyGenerateBundle(
  plugin: Plugin,
  options: NormalizedInputOptions,
  outputOptions: NormalizedOutputOptions,
  pluginContextData: PluginContextData,
): PluginHookWithBindingExt<BindingPluginOptions['generateBundle']> {
  const hook = plugin.generateBundle
  if (!hook) {
    return {}
  }
  const { handler, meta } = normalizeHook(hook)

  return {
    plugin: async (ctx, bundle, isWrite) => {
      const changed = {
        updated: new Set(),
        deleted: new Set(),
      } as ChangedOutputs
      const output = transformToOutputBundle(bundle, changed)
      try {
        await handler.call(
          new PluginContext(options, ctx, plugin, pluginContextData),
          outputOptions,
          output,
          isWrite,
        )
      } catch (e: any) {
        return error(
          logPluginError(e, plugin.name || '<unknown>', {
            hook: 'generateBundle',
          }),
        )
      }
      return collectChangedBundle(changed, output)
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}
export function bindingifyWriteBundle(
  plugin: Plugin,
  options: NormalizedInputOptions,
  outputOptions: NormalizedOutputOptions,
  pluginContextData: PluginContextData,
): PluginHookWithBindingExt<BindingPluginOptions['writeBundle']> {
  const hook = plugin.writeBundle
  if (!hook) {
    return {}
  }
  const { handler, meta } = normalizeHook(hook)

  return {
    plugin: async (ctx, bundle) => {
      const changed = {
        updated: new Set(),
        deleted: new Set(),
      } as ChangedOutputs
      const output = transformToOutputBundle(bundle, changed)
      try {
        await handler.call(
          new PluginContext(options, ctx, plugin, pluginContextData),
          outputOptions,
          output,
        )
      } catch (e: any) {
        return error(
          logPluginError(e, plugin.name || '<unknown>', {
            hook: 'writeBundle',
          }),
        )
      }

      return collectChangedBundle(changed, output)
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}

export function bindingifyCloseBundle(
  plugin: Plugin,
  options: NormalizedInputOptions,
  pluginContextData: PluginContextData,
): PluginHookWithBindingExt<BindingPluginOptions['closeBundle']> {
  const hook = plugin.closeBundle
  if (!hook) {
    return {}
  }
  const { handler, meta } = normalizeHook(hook)

  return {
    plugin: async (ctx) => {
      try {
        await handler.call(
          new PluginContext(options, ctx, plugin, pluginContextData),
        )
      } catch (e: any) {
        return error(
          logPluginError(e, plugin.name || '<unknown>', {
            hook: 'closeBundle',
          }),
        )
      }
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}

export function bindingifyBanner(
  plugin: Plugin,
  options: NormalizedInputOptions,
  pluginContextData: PluginContextData,
): PluginHookWithBindingExt<BindingPluginOptions['banner']> {
  const hook = plugin.banner
  if (!hook) {
    return {}
  }

  const { handler, meta } = normalizeHook(hook)
  return {
    plugin: async (ctx, chunk) => {
      if (typeof handler === 'string') {
        return handler
      }

      try {
        return handler.call(
          new PluginContext(options, ctx, plugin, pluginContextData),
          chunk,
        )
      } catch (e: any) {
        return error(
          logPluginError(e, plugin.name || '<unknown>', {
            hook: 'banner',
          }),
        )
      }
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}

export function bindingifyFooter(
  plugin: Plugin,
  options: NormalizedInputOptions,
  pluginContextData: PluginContextData,
): PluginHookWithBindingExt<BindingPluginOptions['footer']> {
  const hook = plugin.footer
  if (!hook) {
    return {}
  }

  const { handler, meta } = normalizeHook(hook)

  return {
    plugin: async (ctx, chunk) => {
      if (typeof handler === 'string') {
        return handler
      }
      try {
        return handler.call(
          new PluginContext(options, ctx, plugin, pluginContextData),
          chunk,
        )
      } catch (e: any) {
        return error(
          logPluginError(e, plugin.name || '<unknown>', {
            hook: 'footer',
          }),
        )
      }
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}

export function bindingifyIntro(
  plugin: Plugin,
  options: NormalizedInputOptions,
  pluginContextData: PluginContextData,
): PluginHookWithBindingExt<BindingPluginOptions['intro']> {
  const hook = plugin.intro
  if (!hook) {
    return {}
  }

  const { handler, meta } = normalizeHook(hook)

  return {
    plugin: async (ctx, chunk) => {
      if (typeof handler === 'string') {
        return handler
      }

      try {
        return handler.call(
          new PluginContext(options, ctx, plugin, pluginContextData),
          chunk,
        )
      } catch (e: any) {
        return error(
          logPluginError(e, plugin.name || '<unknown>', {
            hook: 'intro',
          }),
        )
      }
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}

export function bindingifyOutro(
  plugin: Plugin,
  options: NormalizedInputOptions,
  pluginContextData: PluginContextData,
): PluginHookWithBindingExt<BindingPluginOptions['outro']> {
  const hook = plugin.outro
  if (!hook) {
    return {}
  }

  const { handler, meta } = normalizeHook(hook)

  return {
    plugin: async (ctx, chunk) => {
      if (typeof handler === 'string') {
        return handler
      }

      try {
        return handler.call(
          new PluginContext(options, ctx, plugin, pluginContextData),
          chunk,
        )
      } catch (e: any) {
        return error(
          logPluginError(e, plugin.name || '<unknown>', {
            hook: 'outro',
          }),
        )
      }
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}
