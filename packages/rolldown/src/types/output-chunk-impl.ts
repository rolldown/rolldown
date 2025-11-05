import type { BindingOutputChunk, ExternalMemoryStatus } from '../binding.cjs';
import { lazyProp } from '../decorators/lazy';
import { transformChunkModules } from '../utils/transform-rendered-chunk';
import { transformToRollupSourceMap } from '../utils/transform-to-rollup-output';
import { getLazyFields, PlainObjectLike } from './plain-object-like';
import type { OutputChunk, RenderedModule, SourceMap } from './rolldown-output';

export class OutputChunkImpl extends PlainObjectLike implements OutputChunk {
  readonly type = 'chunk' as const;

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
    return transformChunkModules(this.bindingChunk.getModules());
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

  __rolldown_external_memory_handle__(
    keepDataAlive?: boolean,
  ): ExternalMemoryStatus {
    if (keepDataAlive) {
      this.#evaluateAllLazyFields();
    }
    return this.bindingChunk.dropInner();
  }

  #evaluateAllLazyFields(): void {
    for (const field of getLazyFields(this)) {
      // Accessing the property triggers lazy evaluation via the @lazyProp decorator.
      const _value = (this as any)[field];
    }
  }
}
