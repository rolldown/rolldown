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
  PluginHookWithBindingMeta,
  bindingifyPluginHookMeta,
} from './bindingify-plugin-hook-meta'

export function bindingifyRenderStart(
  plugin: Plugin,
  options: NormalizedInputOptions,
  outputOptions: NormalizedOutputOptions,
  pluginContextData: PluginContextData,
): PluginHookWithBindingMeta<BindingPluginOptions['renderStart']> {
  const hook = plugin.renderStart
  if (!hook) {
    return [undefined, undefined]
  }
  const { handler, meta } = normalizeHook(hook)

  return [
    async (ctx) => {
      handler.call(
        new PluginContext(options, ctx, plugin, pluginContextData),
        outputOptions,
        options,
      )
    },
    bindingifyPluginHookMeta(meta),
  ]
}

export function bindingifyRenderChunk(
  plugin: Plugin,
  options: NormalizedInputOptions,
  outputOptions: NormalizedOutputOptions,
  pluginContextData: PluginContextData,
): PluginHookWithBindingMeta<BindingPluginOptions['renderChunk']> {
  const hook = plugin.renderChunk
  if (!hook) {
    return [undefined, undefined]
  }
  const { handler, meta } = normalizeHook(hook)

  return [
    async (ctx, code, chunk) => {
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
    bindingifyPluginHookMeta(meta),
  ]
}

export function bindingifyAugmentChunkHash(
  plugin: Plugin,
  options: NormalizedInputOptions,
  pluginContextData: PluginContextData,
): PluginHookWithBindingMeta<BindingPluginOptions['augmentChunkHash']> {
  const hook = plugin.augmentChunkHash
  if (!hook) {
    return [undefined, undefined]
  }
  const { handler, meta } = normalizeHook(hook)

  return [
    async (ctx, chunk) => {
      return await handler.call(
        new PluginContext(options, ctx, plugin, pluginContextData),
        chunk,
      )
    },
    bindingifyPluginHookMeta(meta),
  ]
}

export function bindingifyRenderError(
  plugin: Plugin,
  options: NormalizedInputOptions,
  pluginContextData: PluginContextData,
): PluginHookWithBindingMeta<BindingPluginOptions['renderError']> {
  const hook = plugin.renderError
  if (!hook) {
    return [undefined, undefined]
  }
  const { handler, meta } = normalizeHook(hook)

  return [
    async (ctx, err) => {
      handler.call(
        new PluginContext(options, ctx, plugin, pluginContextData),
        new Error(err),
      )
    },
    bindingifyPluginHookMeta(meta),
  ]
}

export function bindingifyGenerateBundle(
  plugin: Plugin,
  options: NormalizedInputOptions,
  outputOptions: NormalizedOutputOptions,
  pluginContextData: PluginContextData,
): PluginHookWithBindingMeta<BindingPluginOptions['generateBundle']> {
  const hook = plugin.generateBundle
  if (!hook) {
    return [undefined, undefined]
  }
  const { handler, meta } = normalizeHook(hook)

  return [
    async (ctx, bundle, isWrite) => {
      await handler.call(
        new PluginContext(options, ctx, plugin, pluginContextData),
        outputOptions,
        transformToOutputBundle(bundle),
        isWrite,
      )
    },
    bindingifyPluginHookMeta(meta),
  ]
}
export function bindingifyWriteBundle(
  plugin: Plugin,
  options: NormalizedInputOptions,
  outputOptions: NormalizedOutputOptions,
  pluginContextData: PluginContextData,
): PluginHookWithBindingMeta<BindingPluginOptions['writeBundle']> {
  const hook = plugin.writeBundle
  if (!hook) {
    return [undefined, undefined]
  }
  const { handler, meta } = normalizeHook(hook)

  return [
    async (ctx, bundle) => {
      await handler.call(
        new PluginContext(options, ctx, plugin, pluginContextData),
        outputOptions,
        transformToOutputBundle(bundle),
      )
    },
    bindingifyPluginHookMeta(meta),
  ]
}

export function bindingifyBanner(
  plugin: Plugin,
  options: NormalizedInputOptions,
  pluginContextData: PluginContextData,
): PluginHookWithBindingMeta<BindingPluginOptions['banner']> {
  const hook = plugin.banner
  if (!hook) {
    return [undefined, undefined]
  }

  const { handler, meta } = normalizeHook(hook)
  return [
    async (ctx, chunk) => {
      if (typeof handler === 'string') {
        return handler
      }

      return handler.call(
        new PluginContext(options, ctx, plugin, pluginContextData),
        chunk,
      )
    },
    bindingifyPluginHookMeta(meta),
  ]
}

export function bindingifyFooter(
  plugin: Plugin,
  options: NormalizedInputOptions,
  pluginContextData: PluginContextData,
): PluginHookWithBindingMeta<BindingPluginOptions['footer']> {
  const hook = plugin.footer
  if (!hook) {
    return [undefined, undefined]
  }

  const { handler, meta } = normalizeHook(hook)

  return [
    async (ctx, chunk) => {
      if (typeof handler === 'string') {
        return handler
      }

      return handler.call(
        new PluginContext(options, ctx, plugin, pluginContextData),
        chunk,
      )
    },
    bindingifyPluginHookMeta(meta),
  ]
}

export function bindingifyIntro(
  plugin: Plugin,
  options: NormalizedInputOptions,
  pluginContextData: PluginContextData,
): PluginHookWithBindingMeta<BindingPluginOptions['intro']> {
  const hook = plugin.intro
  if (!hook) {
    return [undefined, undefined]
  }

  const { handler, meta } = normalizeHook(hook)

  return [
    async (ctx, chunk) => {
      if (typeof handler === 'string') {
        return handler
      }

      return handler.call(
        new PluginContext(options, ctx, plugin, pluginContextData),
        chunk,
      )
    },
    bindingifyPluginHookMeta(meta),
  ]
}

export function bindingifyOutro(
  plugin: Plugin,
  options: NormalizedInputOptions,
  pluginContextData: PluginContextData,
): PluginHookWithBindingMeta<BindingPluginOptions['outro']> {
  const hook = plugin.outro
  if (!hook) {
    return [undefined, undefined]
  }

  const { handler, meta } = normalizeHook(hook)

  return [
    async (ctx, chunk) => {
      if (typeof handler === 'string') {
        return handler
      }

      return handler.call(
        new PluginContext(options, ctx, plugin, pluginContextData),
        chunk,
      )
    },
    bindingifyPluginHookMeta(meta),
  ]
}
