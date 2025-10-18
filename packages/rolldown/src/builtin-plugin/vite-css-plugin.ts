import type { SourceMapInput } from '..';
import type {
  BindingUrlResolver,
  BindingViteCssPluginConfig,
} from '../binding';
import { bindingifySourcemap } from '../types/sourcemap';
import { BuiltinPlugin } from './utils';

type ViteCssPluginConfig =
  & Omit<
    BindingViteCssPluginConfig,
    'compressCss'
  >
  & {
    compressCss: (
      url: string,
      importer: string,
      resolver: BindingUrlResolver,
    ) => Promise<{
      code: string;
      map?: SourceMapInput;
      modules?: Record<string, string>;
      deps?: Set<string>;
    }>;
  };

export function viteCSSPlugin(
  config?: ViteCssPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin(
    'builtin:vite-css',
    config
      ? {
        ...config,
        async compressCss(
          url: string,
          importer: string,
          resolver: BindingUrlResolver,
        ) {
          let result = await config.compressCss(url, importer, resolver);
          return {
            ...result,
            map: bindingifySourcemap(result.map),
          };
        },
      }
      : undefined,
  );
}
