import type { ObjectHook } from '../plugin'

export function normalizeHook<T extends ObjectHook<Function | string>>(
  hook: T,
): T extends ObjectHook<infer A, infer B>
  ? T extends Function | string
    ? [A, {}]
    : [A, Omit<B, 'handler'>]
  : never {
  if (typeof hook === 'function') {
    // @ts-ignore
    return [hook, {}]
  }

  if (typeof hook === 'object' && hook !== null) {
    const { handler, ...options } = hook
    // @ts-ignore
    return [handler, options]
  }
  // @ts-ignore
  return [hook, {}]
}
