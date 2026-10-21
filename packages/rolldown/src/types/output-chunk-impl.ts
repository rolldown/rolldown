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

  // Undefined after `releaseBindings()`.
  private bindingChunk: BindingOutputChunk | undefined;

  constructor(bindingChunk: BindingOutputChunk) {
    super();
    this.bindingChunk = bindingChunk;
  }

  // Every field is cached before the release, so no getter reaches this then.
  #binding(): BindingOutputChunk {
    if (this.bindingChunk === undefined) {
      throw new Error('This output chunk has already released its binding object');
    }
    return this.bindingChunk;
  }

  @lazyProp
  get fileName(): string {
    return this.#binding().getFileName();
  }

  @lazyProp
  get name(): string {
    return this.#binding().getName();
  }

  @lazyProp
  get exports(): string[] {
    return this.#binding().getExports();
  }

  @lazyProp
  get isEntry(): boolean {
    return this.#binding().getIsEntry();
  }

  @lazyProp
  get facadeModuleId(): string | null {
    return this.#binding().getFacadeModuleId() || null;
  }

  @lazyProp
  get isDynamicEntry(): boolean {
    return this.#binding().getIsDynamicEntry();
  }

  @lazyProp
  get sourcemapFileName(): string | null {
    return this.#binding().getSourcemapFileName() || null;
  }

  @lazyProp
  get preliminaryFileName(): string {
    return this.#binding().getPreliminaryFileName();
  }

  @lazyProp
  get code(): string {
    return this.#binding().getCode();
  }

  @lazyProp
  get modules(): { [id: string]: RenderedModule } {
    const bindingModules = this.#binding().getModules();
    this.#bindingModules = bindingModules;
    return transformChunkModules(bindingModules);
  }

  @lazyProp
  get imports(): string[] {
    return this.#binding().getImports();
  }

  @lazyProp
  get dynamicImports(): string[] {
    return this.#binding().getDynamicImports();
  }

  @lazyProp
  get moduleIds(): string[] {
    return this.#binding().getModuleIds();
  }

  @lazyProp
  get map(): SourceMap | null {
    const mapString = this.#binding().getMap();
    return mapString ? transformToRollupSourceMap(mapString) : null;
  }

  __rolldown_external_memory_handle__(keepDataAlive?: boolean): ExternalMemoryStatus {
    if (keepDataAlive) {
      this.#evaluateAllLazyFields();
      // Threadless WASI only; elsewhere GC finalizers reclaim the module boxes.
      if (shouldEagerlyFreeOutputs()) {
        this.#snapshotModules();
      }
    }
    if (this.bindingChunk === undefined) {
      return { freed: false, reason: 'Memory has already been freed' };
    }
    return this.bindingChunk.dropInner();
  }

  /** @internal Cache every field, then drop the binding wrapper. */
  releaseBindings(): void {
    if (this.bindingChunk === undefined) return;
    this.#evaluateAllLazyFields();
    if (this.#bindingModules !== undefined) this.#snapshotModules();
    this.bindingChunk = undefined;
  }

  #evaluateAllLazyFields(): void {
    for (const field of getLazyFields(this)) {
      // Accessing the property triggers lazy evaluation via the @lazyProp decorator.
      const _value = (this as any)[field];
    }
  }

  // The cached `modules` wrappers read through their own
  // `BindingRenderedModule` boxes, which this chunk's `dropInner()` does not
  // free, so swap in a plain snapshot and release them.
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
