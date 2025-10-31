import type { BindingOutputChunk, ExternalMemoryStatus } from '../binding.cjs';
import { getLazyFields, lazy } from '../decorators/lazy';
import { nonEnumerable } from '../decorators/non-enumerable';
import { transformChunkModules } from '../utils/transform-rendered-chunk';
import { transformToRollupSourceMap } from '../utils/transform-to-rollup-output';
import type { OutputChunk, RenderedModule, SourceMap } from './rolldown-output';

export class OutputChunkImpl implements OutputChunk {
  readonly type = 'chunk' as const;

  constructor(private bindingChunk: BindingOutputChunk) {
  }

  @lazy
  get fileName(): string {
    return this.bindingChunk.getFileName();
  }

  @lazy
  get name(): string {
    return this.bindingChunk.getName();
  }

  @lazy
  get exports(): string[] {
    return this.bindingChunk.getExports();
  }

  @lazy
  get isEntry(): boolean {
    return this.bindingChunk.getIsEntry();
  }

  @lazy
  get facadeModuleId(): string | null {
    return this.bindingChunk.getFacadeModuleId() || null;
  }

  @lazy
  get isDynamicEntry(): boolean {
    return this.bindingChunk.getIsDynamicEntry();
  }

  @lazy
  get sourcemapFileName(): string | null {
    return this.bindingChunk.getSourcemapFileName() || null;
  }

  @lazy
  get preliminaryFileName(): string {
    return this.bindingChunk.getPreliminaryFileName();
  }

  @lazy
  get code(): string {
    return this.bindingChunk.getCode();
  }

  @lazy
  get modules(): { [id: string]: RenderedModule } {
    return transformChunkModules(this.bindingChunk.getModules());
  }

  @lazy
  get imports(): string[] {
    return this.bindingChunk.getImports();
  }

  @lazy
  get dynamicImports(): string[] {
    return this.bindingChunk.getDynamicImports();
  }

  @lazy
  get moduleIds(): string[] {
    return this.bindingChunk.getModuleIds();
  }

  @lazy
  get map(): SourceMap | null {
    const mapString = this.bindingChunk.getMap();
    return mapString ? transformToRollupSourceMap(mapString) : null;
  }

  @nonEnumerable
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
      // Accessing the property triggers lazy evaluation via the @lazy decorator.
      const _value = (this as any)[field];
    }
  }
}
