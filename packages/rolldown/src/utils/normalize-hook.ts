import type { ObjectHook, ObjectHookMeta } from '../plugin';
import type { AnyFn } from '../types/utils';
import { unreachable } from './misc';

/** `cache` exists only on some hooks, see `CacheablePluginHooks`. */
export type NormalizedHookMeta = ObjectHookMeta & { cache?: boolean };

export function normalizeHook<Hook extends ObjectHook<AnyFn | string>>(
  hook: Hook,
): Hook extends ObjectHook<infer RawHook, infer CustomOptions>
  ? Hook extends RawHook
    ? never
    : {
        handler: RawHook;
        options: CustomOptions;
        meta: NormalizedHookMeta;
      }
  : never {
  type Return =
    Hook extends ObjectHook<infer RawHook, infer CustomOptions>
      ? Hook extends RawHook
        ? never
        : {
            handler: RawHook;
            options: CustomOptions;
            meta: NormalizedHookMeta;
          }
      : never;

  if (typeof hook === 'function' || typeof hook === 'string') {
    return {
      handler: hook,
      options: {},
      meta: {},
    } as Return;
  }

  if (typeof hook === 'object' && hook !== null) {
    const { handler, order, ...rest } = hook;
    const { cache, ...options } = rest as typeof rest & { cache?: boolean };
    return {
      handler,
      options,
      meta: {
        order,
        cache,
      },
    } as Return;
  }

  unreachable('Invalid hook type');
}
