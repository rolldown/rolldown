import type { Hook } from '../plugin'
import type { AnyFn, AnyObj } from '../types/utils'

type NotFn<T> = T extends AnyFn ? never : T

export function normalizeHook<H extends Hook<AnyFn, AnyObj>>(
  hook: H,
): H extends Hook<infer Handler, infer Options>
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
