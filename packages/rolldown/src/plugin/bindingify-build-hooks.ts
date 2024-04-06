import { normalizeHook } from '../utils/normalize-hook'
import type { BindingPluginOptions, HookOption } from '../binding'

import type { Hook, Plugin } from './index'
import { RolldownNormalizedInputOptions } from '../options/input-options'
import { AnyFn } from 'src/types/utils'

export function bindingifyBuildStart(
  options: RolldownNormalizedInputOptions,
  hook?: Plugin['buildStart'],
): BindingPluginOptions['buildStart'] {
  if (!hook) {
    return undefined
  }
  const [handler, option] = normalizeHook<Hook<AnyFn, HookOption>>(hook)

  return {
    handler: async (ctx) => {
      handler.call(ctx, options)
    },
    order: option.order,
    sequential: option.sequential,
  }
}

export function bindingifyBuildEnd(
  hook?: Plugin['buildEnd'],
): BindingPluginOptions['buildEnd'] {
  if (!hook) {
    return undefined
  }
  const [handler, option] = normalizeHook<Hook<AnyFn, HookOption>>(hook)

  return {
    handler: async (err) => {
      try {
        handler.call(null, err ?? undefined)
      } catch (error) {
        console.error(error)
      }
    },
    order: option.order,
  }
}

export function bindingifyResolveId(
  hook?: Plugin['resolveId'],
): BindingPluginOptions['resolveId'] {
  if (!hook) {
    return undefined
  }
  const [handler, option] = normalizeHook<Hook<AnyFn, HookOption>>(hook)

  return {
    handler: async (specifier, importer, options) => {
      const ret = await handler.call(
        null,
        specifier,
        importer ?? undefined,
        options,
      )
      if (ret == false || ret == null) {
        return
      }
      if (typeof ret === 'string') {
        return {
          id: ret,
        }
      }
      return ret
    },
    order: option.order,
  }
}

export function bindingifyTransform(
  hook?: Plugin['transform'],
): BindingPluginOptions['transform'] {
  if (!hook) {
    return undefined
  }
  const [handler, option] = normalizeHook<Hook<AnyFn, HookOption>>(hook)

  return {
    handler: async (code, id) => {
      const ret = await handler.call(null, code, id)

      if (ret == null) {
        return
      }

      const retCode = typeof ret === 'string' ? ret : ret.code
      const retMap = typeof ret === 'string' ? undefined : ret.map

      return {
        code: retCode,
        map: retMap ?? undefined,
      }
    },
    order: option.order,
  }
}

export function bindingifyLoad(
  hook?: Plugin['load'],
): BindingPluginOptions['load'] {
  if (!hook) {
    return undefined
  }
  const [handler, option] = normalizeHook<Hook<AnyFn, HookOption>>(hook)

  return {
    handler: async (id) => {
      const ret = await handler.call(null, id)

      if (ret == null) {
        return
      }

      const retCode = typeof ret === 'string' ? ret : ret.code
      const retMap = typeof ret === 'string' ? undefined : ret.map

      return {
        code: retCode,
        map: retMap ?? undefined,
      }
    },
    order: option.order,
  }
}

export function bindingifyRenderChunk(
  hook?: Plugin['renderChunk'],
): BindingPluginOptions['renderChunk'] {
  if (!hook) {
    return undefined
  }
  const [handler, option] = normalizeHook<Hook<AnyFn, HookOption>>(hook)

  return {
    handler: async (code, chunk) => {
      const ret = await handler.call(null, code, chunk)

      if (ret == null) {
        return
      }

      return {
        code: ret,
      }
    },
    order: option.order,
  }
}
