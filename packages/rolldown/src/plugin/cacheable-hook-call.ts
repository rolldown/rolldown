import type { BindingPluginContext } from '../binding.cjs';
import { isPromiseLike } from '../utils/misc';
import type { ModuleInfoAccess } from './module-info-access';
import type { PluginContextData, RecordedHookCall } from './plugin-context-data';

/**
 * Calls the handler of a `resolveId`, `resolveDynamicImport`, `load` or `transform` hook. With
 * the persistent cache, it then gives what the hook did through `this.getModuleInfo` and
 * `this.resolve` to the cache. `access` is `undefined` without the cache.
 *
 * Without the cache, it returns what the handler returns, so the caller awaits the same value
 * as with a direct call.
 */
export function callCacheableHook<T>(
  data: PluginContextData,
  context: BindingPluginContext,
  access: ModuleInfoAccess | undefined,
  hook: RecordedHookCall,
  call: () => T,
): T | Promise<Awaited<T>> {
  if (!access) {
    return call();
  }
  return callAndRecord(data, context, access, hook, call);
}

/**
 * When the handler returns no promise, the hook call has already ended. `access` closes at once,
 * so code that runs during the `await` below does not count as a use of this hook call.
 */
async function callAndRecord<T>(
  data: PluginContextData,
  context: BindingPluginContext,
  access: ModuleInfoAccess,
  hook: RecordedHookCall,
  call: () => T,
): Promise<Awaited<T>> {
  let ret: Awaited<T>;
  let recorded = false;
  try {
    const pending = call();
    if (!isPromiseLike(pending)) {
      data.recordModuleInfoAccess(context, hook, access);
      recorded = true;
    }
    ret = await pending;
  } catch (error) {
    access.close();
    throw error;
  }
  if (!recorded) {
    data.recordModuleInfoAccess(context, hook, access);
  }
  return ret;
}
