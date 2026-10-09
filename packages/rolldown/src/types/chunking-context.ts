import type { BindingChunkingContext } from '../binding.cjs';
import type { PluginContextData } from '../plugin/plugin-context-data';
import { shouldEagerlyFreeOutputs } from '../utils/threadless-free';
import { snapshotModuleInfo, transformModuleInfo } from '../utils/transform-module-info';
import type { ModuleInfo } from './module-info';

export class ChunkingContextImpl {
  private moduleInfoCache: Map<string, ModuleInfo> | undefined = new Map();

  constructor(
    private context: BindingChunkingContext,
    private pluginContextData: PluginContextData,
  ) {}

  /**
   * Adopts the fresh box of a new `name` batch (see `getChunkingContext` in
   * `bindingify-output-options.ts`). The cache holds plain values, so it
   * survives the swap.
   */
  useBindingContext(context: BindingChunkingContext): void {
    this.context = context;
  }

  clearModuleInfoCache(): void {
    this.moduleInfoCache = undefined;
  }

  getModuleInfo(moduleId: string): ModuleInfo | null {
    const cached = this.moduleInfoCache?.get(moduleId);
    if (cached) {
      return cached;
    }
    const bindingInfo = this.context.getModuleInfo(moduleId);
    if (bindingInfo) {
      const option = this.pluginContextData.getModuleOption(moduleId);
      // Each call mints a box holding the module's full source; a threadless
      // WASI host may never run GC finalizers, so hand out a snapshot and
      // release the box.
      const info = shouldEagerlyFreeOutputs()
        ? snapshotModuleInfo(bindingInfo, option)
        : transformModuleInfo(bindingInfo, option);
      // `moduleSideEffects` reads and writes the shared module-option store
      // rather than the native box, so the write-through survives both the
      // snapshot and the cache.
      Object.defineProperty(info, 'moduleSideEffects', {
        get: () => option.moduleSideEffects,
        set: (moduleSideEffects: ModuleInfo['moduleSideEffects']) => {
          option.moduleSideEffects = moduleSideEffects;
          option.invalidate = true;
        },
      });
      this.moduleInfoCache?.set(moduleId, info);
      return info;
    }
    return null;
  }
}
