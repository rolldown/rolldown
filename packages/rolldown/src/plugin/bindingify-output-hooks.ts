import { normalizeHook } from '../utils/normalize-hook'
import type { BindingPluginOptions } from '../binding'
import type { NormalizedInputOptions } from '../options/normalized-input-options'
import type { Plugin } from './index'
import { transformToOutputBundle } from '../utils/transform-to-rollup-output'
import { PluginContext } from './plugin-context'
import { bindingifySourcemap } from '../types/sourcemap'
import { NormalizedOutputOptions } from '../options/normalized-output-options'
import { PluginContextData } from './plugin-context-data'
import {
  PluginHookWithBindingExt,
  bindingifyPluginHookMeta,
} from './bindingify-plugin-hook-meta'

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
      handler.call(
        new PluginContext(options, ctx, plugin, pluginContextData),
        outputOptions,
        options,
      )
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
      const ret = await handler.call(
        new PluginContext(options, ctx, plugin, pluginContextData),
        code,
        chunk,
        outputOptions,
      )

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
      return await handler.call(
        new PluginContext(options, ctx, plugin, pluginContextData),
        chunk,
      )
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
      handler.call(
        new PluginContext(options, ctx, plugin, pluginContextData),
        new Error(err),
      )
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
      await handler.call(
        new PluginContext(options, ctx, plugin, pluginContextData),
        outputOptions,
        transformToOutputBundle(bundle),
        isWrite,
      )
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
      await handler.call(
        new PluginContext(options, ctx, plugin, pluginContextData),
        outputOptions,
        transformToOutputBundle(bundle),
      )
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
      await handler.call(
        new PluginContext(options, ctx, plugin, pluginContextData),
      )
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

      return handler.call(
        new PluginContext(options, ctx, plugin, pluginContextData),
        chunk,
      )
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

      return handler.call(
        new PluginContext(options, ctx, plugin, pluginContextData),
        chunk,
      )
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

      return handler.call(
        new PluginContext(options, ctx, plugin, pluginContextData),
        chunk,
      )
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

      return handler.call(
        new PluginContext(options, ctx, plugin, pluginContextData),
        chunk,
      )
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}
