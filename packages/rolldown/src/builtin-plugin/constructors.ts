import type { PartialResolvedId } from '..';
import type {
  BindingAssetPluginConfig,
  BindingBuildImportAnalysisPluginConfig,
  BindingBuiltinPluginName,
  BindingDynamicImportVarsPluginConfig,
  BindingHookResolveIdOutput,
  BindingImportGlobPluginConfig,
  BindingIsolatedDeclarationPluginConfig,
  BindingJsonPluginConfig,
  BindingManifestPluginConfig,
  BindingMfManifest,
  BindingModuleFederationPluginOption,
  BindingModulePreloadPolyfillPluginConfig,
  BindingOxcRuntimePluginConfig,
  BindingRemote,
  BindingReporterPluginConfig,
  BindingViteResolvePluginConfig,
  BindingWasmHelperPluginConfig,
} from '../binding';
import type { StringOrRegExp } from '../types/utils';
import { normalizedStringOrRegex } from '../utils/normalize-string-or-regex';
import { makeBuiltinPluginCallable } from './utils';

export class BuiltinPlugin {
  constructor(
    public name: BindingBuiltinPluginName,
    // NOTE: has `_` to avoid conflict with `options` hook
    public _options?: unknown,
  ) {}
}

export function modulePreloadPolyfillPlugin(
  config?: BindingModulePreloadPolyfillPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:module-preload-polyfill', config);
}

type DynamicImportVarsPluginConfig =
  & Omit<
    BindingDynamicImportVarsPluginConfig,
    'include' | 'exclude'
  >
  & {
    include?: StringOrRegExp | StringOrRegExp[];
    exclude?: StringOrRegExp | StringOrRegExp[];
  };

export function dynamicImportVarsPlugin(
  config?: DynamicImportVarsPluginConfig,
): BuiltinPlugin {
  if (config) {
    config.include = normalizedStringOrRegex(config.include);
    config.exclude = normalizedStringOrRegex(config.exclude);
  }
  return new BuiltinPlugin('builtin:dynamic-import-vars', config);
}

export function importGlobPlugin(
  config?: BindingImportGlobPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:import-glob', config);
}

export function reporterPlugin(
  config?: BindingReporterPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:reporter', config);
}

export function manifestPlugin(
  config?: BindingManifestPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:manifest', config);
}

export function wasmHelperPlugin(
  config?: BindingWasmHelperPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:wasm-helper', config);
}

export function wasmFallbackPlugin(): BuiltinPlugin {
  const builtinPlugin = new BuiltinPlugin('builtin:wasm-fallback');
  return makeBuiltinPluginCallable(builtinPlugin);
}

export function loadFallbackPlugin(): BuiltinPlugin {
  return new BuiltinPlugin('builtin:load-fallback');
}

export function jsonPlugin(config?: BindingJsonPluginConfig): BuiltinPlugin {
  const builtinPlugin = new BuiltinPlugin('builtin:json', config);
  return makeBuiltinPluginCallable(builtinPlugin);
}

export function buildImportAnalysisPlugin(
  config: BindingBuildImportAnalysisPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:build-import-analysis', config);
}

type ViteResolvePluginConfig =
  & Omit<
    BindingViteResolvePluginConfig,
    'finalizeBareSpecifier'
  >
  & {
    finalizeBareSpecifier?: (
      id: string,
      importer: string | undefined,
      scan: boolean,
    ) => Promise<PartialResolvedId>;
  };

export function viteResolvePlugin(
  config: ViteResolvePluginConfig,
): BuiltinPlugin {
  if (config.finalizeBareSpecifier) {
    const finalizeBareSpecifier = config.finalizeBareSpecifier;
    const newFinalizeBareSpecifier = async (
      id: string,
      importer: string | undefined,
      scan: boolean,
    ) => {
      const ret = await finalizeBareSpecifier(id, importer, scan);

      if (typeof ret === 'string') {
        return { id: ret };
      }
      const result: BindingHookResolveIdOutput = {
        id: ret.id,
        external: ret.external,
      };

      if (ret.moduleSideEffects !== null) {
        result.moduleSideEffects = ret.moduleSideEffects;
      }

      return result;
    };
    config.finalizeBareSpecifier = newFinalizeBareSpecifier as any;
  }
  const builtinPlugin = new BuiltinPlugin('builtin:vite-resolve', config);
  return makeBuiltinPluginCallable(builtinPlugin);
}

type ModuleFederationPluginOption =
  & Omit<
    BindingModuleFederationPluginOption,
    'remotes'
  >
  & {
    remotes?: Record<string, string | BindingRemote>;
    manifest?: boolean | BindingMfManifest;
  };

export function moduleFederationPlugin(
  config: ModuleFederationPluginOption,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:module-federation', {
    ...config,
    remotes: config.remotes &&
      Object.entries(config.remotes).map(([name, remote]) => {
        if (typeof remote === 'string') {
          const [entryGlobalName] = remote.split('@');
          const entry = remote.replace(entryGlobalName + '@', '');
          return { entry, name, entryGlobalName };
        }
        return {
          ...remote,
          name: remote.name ?? name,
        };
      }),
    manifest: config.manifest === false
      ? undefined
      : config.manifest === true
      ? {}
      : config.manifest,
  });
}

export function isolatedDeclarationPlugin(
  config?: BindingIsolatedDeclarationPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:isolated-declaration', config);
}

export function assetPlugin(
  config?: BindingAssetPluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:asset', config);
}

export function webWorkerPostPlugin(): BuiltinPlugin {
  return new BuiltinPlugin('builtin:web-worker-post');
}

export function oxcRuntimePlugin(
  config?: BindingOxcRuntimePluginConfig,
): BuiltinPlugin {
  return new BuiltinPlugin('builtin:oxc-runtime', config);
}
