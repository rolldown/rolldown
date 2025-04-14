import { isPromise } from 'node:util/types';
import { HookFilterExtension, Plugin, RolldownPluginOption } from '..';

type OverrideFilterObject = {
  transform?: HookFilterExtension<'transform'>['filter'];
  resolveId?: HookFilterExtension<'resolveId'>['filter'];
  load?: HookFilterExtension<'load'>['filter'];
};
export function withFilter<A, T extends RolldownPluginOption<A>>(
  pluginOption: T,
  filter_obj: OverrideFilterObject,
): T {
  if (isPromise(pluginOption)) {
    return pluginOption.then((p) => withFilter(p, filter_obj)) as T;
  }
  if (pluginOption == false || pluginOption == null) {
    return pluginOption;
  }
  if (Array.isArray(pluginOption)) {
    return pluginOption.map((p) => withFilter(p, filter_obj)) as T;
  }
  // TODO: check builtin plugin and parallel plugin
  let plugin = pluginOption as Plugin<A>;
  Object.keys(plugin).forEach((key) => {
    switch (key) {
      case 'transform':
      case 'resolveId':
      case 'load':
        if (!plugin[key]) {
          return;
        }
        if (typeof plugin[key] === 'object') {
          plugin[key].filter = filter_obj[key] ?? plugin[key].filter;
        } else {
          // We could either remove the `@ts-expect-error` and duplicate case `transform`, `resolveId` and `load`
          // or use `@ts-expect-error` to just ignore the type error
          // Prefer simplicity because we already checked before
          plugin[key] = {
            // @ts-expect-error
            handler: plugin[key],
            // @ts-expect-error
            filter: filter_obj[key],
          };
        }
        break;
      default:
        break;
    }
  });
  return plugin as T;
}
