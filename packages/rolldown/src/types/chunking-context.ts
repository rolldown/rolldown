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
   * One context serves a whole chunking pass so its module-info cache spans
   * every group, but each group's `name` batch is handed a freshly minted
   * context box and `batchName` releases that box when its loop ends. The
   * reused context therefore adopts the live box at the start of each batch.
   * The cache holds plain JavaScript values, not pending native reads, so it
   * is unaffected by the swap.
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
      // Each call mints a fresh module-info box retaining the module's full
      // source, and the threadless flavor never runs GC finalizers, so hand
      // out a plain-data snapshot and release the box immediately. Both shapes
      // are plain objects over the same module-option store, so either can be
      // cached and carry the live `moduleSideEffects` accessor below.
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
