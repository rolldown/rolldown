import { normalizeHook } from '../utils/normalize-hook'
import {
  ChangedOutputs,
  collectChangedBundle,
  transformToOutputBundle,
} from '../utils/transform-to-rollup-output'
import { PluginContext } from './plugin-context'
import { bindingifySourcemap } from '../types/sourcemap'
import {
  PluginHookWithBindingExt,
  bindingifyPluginHookMeta,
} from './bindingify-plugin-hook-meta'
import { transformToRenderedModule } from '../utils/transform-rendered-module'
import { NormalizedInputOptionsImpl } from '../options/normalized-input-options'
import { NormalizedOutputOptionsImpl } from '../options/normalized-output-options'
import type { BindingifyPluginArgs } from './bindingify-plugin'
import type { BindingPluginOptions } from '../binding'

export function bindingifyRenderStart(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['renderStart']> {
  const hook = args.plugin.renderStart
  if (!hook) {
    return {}
  }
  const { handler, meta } = normalizeHook(hook)

  return {
    plugin: async (ctx, opts) => {
      handler.call(
        new PluginContext(
          ctx,
          args.plugin,
          args.pluginContextData,
          args.onLog,
          args.logLevel,
        ),
        new NormalizedOutputOptionsImpl(opts),
        new NormalizedInputOptionsImpl(opts, args.onLog),
      )
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}
export function bindingifyRenderChunk(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['renderChunk']> {
  const hook = args.plugin.renderChunk
  if (!hook) {
    return {}
  }
  const { handler, meta } = normalizeHook(hook)

  return {
    plugin: async (ctx, code, chunk, opts) => {
      Object.entries(chunk.modules).forEach(([key, module]) => {
        chunk.modules[key] = transformToRenderedModule(module)
      })

      const ret = await handler.call(
        new PluginContext(
          ctx,
          args.plugin,
          args.pluginContextData,
          args.onLog,
          args.logLevel,
        ),
        code,
        chunk,
        new NormalizedOutputOptionsImpl(opts),
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
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['augmentChunkHash']> {
  const hook = args.plugin.augmentChunkHash
  if (!hook) {
    return {}
  }
  const { handler, meta } = normalizeHook(hook)

  return {
    plugin: async (ctx, chunk) => {
      Object.entries(chunk.modules).forEach(([key, module]) => {
        chunk.modules[key] = transformToRenderedModule(module)
      })

      return await handler.call(
        new PluginContext(
          ctx,
          args.plugin,
          args.pluginContextData,
          args.onLog,
          args.logLevel,
        ),
        chunk,
      )
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}

export function bindingifyRenderError(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['renderError']> {
  const hook = args.plugin.renderError
  if (!hook) {
    return {}
  }
  const { handler, meta } = normalizeHook(hook)

  return {
    plugin: async (ctx, err) => {
      handler.call(
        new PluginContext(
          ctx,
          args.plugin,
          args.pluginContextData,
          args.onLog,
          args.logLevel,
        ),
        new Error(err),
      )
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}

export function bindingifyGenerateBundle(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['generateBundle']> {
  const hook = args.plugin.generateBundle
  if (!hook) {
    return {}
  }
  const { handler, meta } = normalizeHook(hook)

  return {
    plugin: async (ctx, bundle, isWrite, opts) => {
      const changed = {
        updated: new Set(),
        deleted: new Set(),
      } as ChangedOutputs
      const output = transformToOutputBundle(bundle, changed)
      await handler.call(
        new PluginContext(
          ctx,
          args.plugin,
          args.pluginContextData,
          args.onLog,
          args.logLevel,
        ),
        new NormalizedOutputOptionsImpl(opts),
        output,
        isWrite,
      )
      return collectChangedBundle(changed, output)
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}

export function bindingifyWriteBundle(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['writeBundle']> {
  const hook = args.plugin.writeBundle
  if (!hook) {
    return {}
  }
  const { handler, meta } = normalizeHook(hook)

  return {
    plugin: async (ctx, bundle, opts) => {
      const changed = {
        updated: new Set(),
        deleted: new Set(),
      } as ChangedOutputs
      const output = transformToOutputBundle(bundle, changed)
      await handler.call(
        new PluginContext(
          ctx,
          args.plugin,
          args.pluginContextData,
          args.onLog,
          args.logLevel,
        ),
        new NormalizedOutputOptionsImpl(opts),
        output,
      )
      return collectChangedBundle(changed, output)
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}

export function bindingifyCloseBundle(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['closeBundle']> {
  const hook = args.plugin.closeBundle
  if (!hook) {
    return {}
  }
  const { handler, meta } = normalizeHook(hook)

  return {
    plugin: async (ctx) => {
      await handler.call(
        new PluginContext(
          ctx,
          args.plugin,
          args.pluginContextData,
          args.onLog,
          args.logLevel,
        ),
      )
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}

export function bindingifyBanner(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['banner']> {
  const hook = args.plugin.banner
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
        new PluginContext(
          ctx,
          args.plugin,
          args.pluginContextData,
          args.onLog,
          args.logLevel,
        ),
        chunk,
      )
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}

export function bindingifyFooter(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['footer']> {
  const hook = args.plugin.footer
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
        new PluginContext(
          ctx,
          args.plugin,
          args.pluginContextData,
          args.onLog,
          args.logLevel,
        ),
        chunk,
      )
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}

export function bindingifyIntro(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['intro']> {
  const hook = args.plugin.intro
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
        new PluginContext(
          ctx,
          args.plugin,
          args.pluginContextData,
          args.onLog,
          args.logLevel,
        ),
        chunk,
      )
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}

export function bindingifyOutro(
  args: BindingifyPluginArgs,
): PluginHookWithBindingExt<BindingPluginOptions['outro']> {
  const hook = args.plugin.outro
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
        new PluginContext(
          ctx,
          args.plugin,
          args.pluginContextData,
          args.onLog,
          args.logLevel,
        ),
        chunk,
      )
    },
    meta: bindingifyPluginHookMeta(meta),
  }
}
