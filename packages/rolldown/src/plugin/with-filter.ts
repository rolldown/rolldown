import type { HookFilterExtension, Plugin, RolldownPluginOption } from '..';
import type { StringOrRegExp } from '../types/utils';
import { arraify, isPromiseLike } from '../utils/misc';

type OverrideFilterObject = {
  transform?: HookFilterExtension<'transform'>['filter'];
  resolveId?: HookFilterExtension<'resolveId'>['filter'];
  load?: HookFilterExtension<'load'>['filter'];
  pluginNamePattern?: StringOrRegExp[];
};

function withFilterImpl<A, T extends RolldownPluginOption<A>>(
  pluginOption: T,
  filterObjectList: OverrideFilterObject[],
): T {
  if (isPromiseLike(pluginOption)) {
    return pluginOption.then((p) => withFilter(p, filterObjectList)) as T;
  }
  if (pluginOption == false || pluginOption == null) {
    return pluginOption;
  }
  if (Array.isArray(pluginOption)) {
    return pluginOption.map((p) => withFilter(p, filterObjectList)) as T;
  }
  // TODO: check builtin plugin and parallel plugin
  let plugin = pluginOption as Plugin<A>;
  let filterObjectIndex = findMatchedFilterObject(plugin.name, filterObjectList);
  if (filterObjectIndex === -1) {
    return plugin as T;
  }
  let filterObject = filterObjectList[filterObjectIndex];
  Object.keys(plugin).forEach((key) => {
    switch (key) {
      case 'transform':
      case 'resolveId':
      case 'load':
        if (!plugin[key]) {
          return;
        }
        if (typeof plugin[key] === 'object') {
          plugin[key].filter = filterObject[key] ?? plugin[key].filter;
        } else {
          // We could either remove the `@ts-expect-error` and duplicate case `transform`, `resolveId` and `load`
          // or use `@ts-expect-error` to just ignore the type error
          // Prefer simplicity because we already checked before
          plugin[key] = {
            // @ts-expect-error
            handler: plugin[key],
            // @ts-expect-error
            filter: filterObject[key],
          };
        }
        break;
      default:
        break;
    }
  });
  return plugin as T;
}

/**
 * A helper function to add plugin hook filters to a plugin or an array of plugins.
 *
 * @example
 * ```ts
 * import yaml from '@rollup/plugin-yaml';
 * import { defineConfig } from 'rolldown';
 * import { withFilter } from 'rolldown/filter';
 *
 * export default defineConfig({
 *   plugins: [
 *     // Run the transform hook of the `yaml` plugin
 *     // only for modules which end in `.yaml`
 *     withFilter(
 *       yaml({}),
 *       { transform: { id: /\.yaml$/ } },
 *     ),
 *   ],
 * });
 * ```
 *
 * @category Config
 */
export function withFilter<A, T extends RolldownPluginOption<A>>(
  pluginOption: T,
  filterObject: OverrideFilterObject | OverrideFilterObject[],
): T {
  return withFilterImpl(pluginOption, arraify(filterObject));
}

function findMatchedFilterObject(
  pluginName: string,
  overrideFilterObjectList: OverrideFilterObject[],
): number {
  if (
    overrideFilterObjectList.length === 1 &&
    overrideFilterObjectList[0].pluginNamePattern === undefined
  ) {
    return 0;
  }

  for (let i = 0; i < overrideFilterObjectList.length; i++) {
    for (let j = 0; j < (overrideFilterObjectList[i].pluginNamePattern ?? []).length; j++) {
      let pattern = overrideFilterObjectList[i].pluginNamePattern![j];
      if (typeof pattern === 'string' && pattern === pluginName) {
        return i;
      } else if (pattern instanceof RegExp && pattern.test(pluginName)) {
        return i;
      }
    }
  }
  return -1;
}
