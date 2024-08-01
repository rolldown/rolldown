import type { ObjectHook } from '../plugin'
import type { AnyFn, AnyObj } from '../types/utils'

export function normalizeHook<T extends AnyFn | string>(
  hook: ObjectHook<T, AnyObj>,
): [T, AnyObj] {
  if (typeof hook === 'function') {
    return [hook, {}]
  }

  if (typeof hook === 'object') {
    const { handler, ...options } = hook

    return [handler, options]
  }

  return [hook, {}]
}
