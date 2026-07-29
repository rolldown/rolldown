import type { BindingModules, BindingOutputChunk, ExternalMemoryStatus } from '../binding.cjs';
import { lazyProp } from '../decorators/lazy';
import { shouldEagerlyFreeOutputs } from '../utils/threadless-free';
import { transformChunkModules } from '../utils/transform-rendered-chunk';
import { transformToRollupSourceMap } from '../utils/transform-to-rollup-output';
import { getLazyFields, PlainObjectLike } from './plain-object-like';
import type { OutputChunk, RenderedModule, SourceMap } from './rolldown-output';

export class OutputChunkImpl extends PlainObjectLike implements OutputChunk {
  readonly type = 'chunk' as const;

  // The `BindingRenderedModule` boxes behind the materialized `modules` map,
  // kept so `__rolldown_external_memory_handle__(true)` can release them after
  // snapshotting their data (see `#snapshotModules`).
  #bindingModules: BindingModules | undefined;

  constructor(private bindingChunk: BindingOutputChunk) {
    super();
  }

  @lazyProp
  get fileName(): string {
    return this.bindingChunk.getFileName();
  }

  @lazyProp
  get name(): string {
    return this.bindingChunk.getName();
  }

  @lazyProp
  get exports(): string[] {
    return this.bindingChunk.getExports();
  }

  @lazyProp
  get isEntry(): boolean {
    return this.bindingChunk.getIsEntry();
  }

  @lazyProp
  get facadeModuleId(): string | null {
    return this.bindingChunk.getFacadeModuleId() || null;
  }

  @lazyProp
  get isDynamicEntry(): boolean {
    return this.bindingChunk.getIsDynamicEntry();
  }

  @lazyProp
  get sourcemapFileName(): string | null {
    return this.bindingChunk.getSourcemapFileName() || null;
  }

  @lazyProp
  get preliminaryFileName(): string {
    return this.bindingChunk.getPreliminaryFileName();
  }

  @lazyProp
  get code(): string {
    return this.bindingChunk.getCode();
  }

  @lazyProp
  get modules(): { [id: string]: RenderedModule } {
    const bindingModules = this.bindingChunk.getModules();
    this.#bindingModules = bindingModules;
    return transformChunkModules(bindingModules);
  }

  @lazyProp
  get imports(): string[] {
    return this.bindingChunk.getImports();
  }

  @lazyProp
  get dynamicImports(): string[] {
    return this.bindingChunk.getDynamicImports();
  }

  @lazyProp
  get moduleIds(): string[] {
    return this.bindingChunk.getModuleIds();
  }

  @lazyProp
  get map(): SourceMap | null {
    const mapString = this.bindingChunk.getMap();
    return mapString ? transformToRollupSourceMap(mapString) : null;
  }

  __rolldown_external_memory_handle__(keepDataAlive?: boolean): ExternalMemoryStatus {
    if (keepDataAlive) {
      this.#evaluateAllLazyFields();
      // Threadless WASI only: on native the historical keepDataAlive shape
      // (live-getter wrappers whose boxes are reclaimed by GC finalizers)
      // keeps working and stays byte-identical for existing consumers.
      if (shouldEagerlyFreeOutputs()) {
        this.#snapshotModules();
      }
    }
    return this.bindingChunk.dropInner();
  }

  #evaluateAllLazyFields(): void {
    for (const field of getLazyFields(this)) {
      // Accessing the property triggers lazy evaluation via the @lazyProp decorator.
      const _value = (this as any)[field];
    }
  }

  // The cached `modules` map holds live-getter wrappers whose every read calls
  // into a `BindingRenderedModule` box (its own native `Arc` clone, unaffected
  // by this chunk's `dropInner()`). On the threadless-WASI flavor the GC
  // finalizers that would reclaim those boxes never run (and reads after the
  // owning instance is disposed would throw), so replace each wrapper with a
  // plain snapshot of its three fields and release the boxes eagerly.
  #snapshotModules(): void {
    const modules = this.modules;
    for (const key of Object.keys(modules)) {
      const rendered = modules[key];
      modules[key] = {
        code: rendered.code,
        renderedLength: rendered.renderedLength,
        renderedExports: rendered.renderedExports,
      };
    }
    const bindingModules = this.#bindingModules;
    if (bindingModules !== undefined) {
      for (const box of bindingModules.values) {
        box.dropInner();
      }
      this.#bindingModules = undefined;
    }
  }
}
