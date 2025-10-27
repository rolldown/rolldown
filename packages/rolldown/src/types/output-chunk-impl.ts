import type { BindingOutputChunk } from '../binding';
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
    return this.bindingChunk.fileName;
  }

  @lazy
  get name(): string {
    return this.bindingChunk.name;
  }

  @lazy
  get exports(): string[] {
    return this.bindingChunk.exports;
  }

  @lazy
  get isEntry(): boolean {
    return this.bindingChunk.isEntry;
  }

  @lazy
  get facadeModuleId(): string | null {
    return this.bindingChunk.facadeModuleId || null;
  }

  @lazy
  get isDynamicEntry(): boolean {
    return this.bindingChunk.isDynamicEntry;
  }

  @lazy
  get sourcemapFileName(): string | null {
    return this.bindingChunk.sourcemapFileName || null;
  }

  @lazy
  get preliminaryFileName(): string {
    return this.bindingChunk.preliminaryFileName;
  }

  @lazy
  get code(): string {
    return this.bindingChunk.code;
  }

  @lazy
  get modules(): { [id: string]: RenderedModule } {
    return transformChunkModules(this.bindingChunk.modules);
  }

  @lazy
  get imports(): string[] {
    return this.bindingChunk.imports;
  }

  @lazy
  get dynamicImports(): string[] {
    return this.bindingChunk.dynamicImports;
  }

  @lazy
  get moduleIds(): string[] {
    return this.bindingChunk.moduleIds;
  }

  @lazy
  get map(): SourceMap | null {
    return this.bindingChunk.map
      ? transformToRollupSourceMap(this.bindingChunk.map)
      : null;
  }

  @nonEnumerable
  __rolldown_external_memory_handle__(keepDataAlive?: boolean): boolean {
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
