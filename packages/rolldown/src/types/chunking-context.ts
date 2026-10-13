import type { BindingChunkingContext } from '../binding.cjs';
import type { PluginContextData } from '../plugin/plugin-context-data';
import { shouldEagerlyFreeOutputs } from '../utils/threadless-free';
import { snapshotModuleInfo, transformModuleInfo } from '../utils/transform-module-info';
import type { ModuleInfo } from './module-info';

export class ChunkingContextImpl {
  private moduleInfoCache: Map<string, ModuleInfo> | undefined = new Map();
  // Undefined after `releaseBinding()`.
  private context: BindingChunkingContext | undefined;

  constructor(
    context: BindingChunkingContext,
    private pluginContextData: PluginContextData,
  ) {
    this.context = context;
  }

  // A TS-private method, not `#private`, as in `PluginContextImpl`: a context
  // reached through a Proxy or a derived object must keep working.
  private binding(): BindingChunkingContext {
    if (this.context === undefined) {
      throw new Error('This chunking context has already released its binding object');
    }
    return this.context;
  }

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

  /**
   * Drop the binding wrapper at the build's terminal release (see
   * `PluginContextData.retainContextBox`): its methods keep the binding
   * instance that made it alive, so a context a classifier keeps would keep
   * that instance's memory too. Module infos already handed out stay readable.
   */
  releaseBinding(): void {
    this.context = undefined;
  }

  getModuleInfo(moduleId: string): ModuleInfo | null {
    const cached = this.moduleInfoCache?.get(moduleId);
    if (cached) {
      return cached;
    }
    const bindingInfo = this.binding().getModuleInfo(moduleId);
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
