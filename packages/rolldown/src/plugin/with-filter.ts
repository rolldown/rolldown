import { HookFilterExtension, Plugin } from '..';

type OverrideFilterObject = {
  transform?: HookFilterExtension<'transform'>['filter'];
  resolveId?: HookFilterExtension<'resolveId'>['filter'];
  load?: HookFilterExtension<'load'>['filter'];
};
// TODO: maybe we should also change the name as well ?
// my proposal is `${import.meta.filename}@${name}`
export function withFilter<A, T extends Plugin<A>>(
  plugin: T,
  filter_obj: OverrideFilterObject,
): T {
  plugin['transform'];
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
  return plugin;
}
