import { AnyFn } from '../types/utils'
import type { ObjectHook, ObjectHookMeta } from '../plugin'
import { unreachable } from './misc'

export function normalizeHook<Hook extends ObjectHook<AnyFn | string>>(
  hook: Hook,
) {
  type Return =
    Hook extends ObjectHook<infer RawHook, infer CustomOptions>
      ? {
          handler: RawHook
          options: CustomOptions
          meta: ObjectHookMeta
        }
      : never

  if (typeof hook === 'function' || typeof hook === 'string') {
    return {
      handler: hook,
      options: {},
      meta: {},
    } as Return
  } else if (typeof hook === 'object' && hook !== null) {
    const { handler, order, ...options } = hook
    return {
      handler,
      options,
      meta: {
        order,
      },
    } as Return
  }

  unreachable('Invalid hook type')
}
