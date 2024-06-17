import type { ObjectHook } from '../plugin'
import type { AnyFn, AnyObj } from '../types/utils'

type NotFn<T> = T extends AnyFn ? never : T

export function normalizeHook<H extends ObjectHook<AnyFn, AnyObj>>(
  hook: H,
): H extends ObjectHook<infer Handler, infer Options>
  ? [Handler, NotFn<Options>]
  : never {
  if (typeof hook === 'function') {
    // @ts-expect-error
    return [hook, {}]
  }
  const { handler, ...options } = hook

  // @ts-expect-error
  return [handler, options]
}
