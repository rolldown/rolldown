import type { BindingRenderedChunk } from '../binding.cjs';
import { lazyProp } from '../decorators/lazy';
import { PlainObjectLike } from '../types/plain-object-like';
import type { RenderedChunk, RenderedModule } from '../types/rolldown-output';
import { transformToRenderedModule } from './transform-rendered-module';

class RenderedChunkImpl extends PlainObjectLike implements RenderedChunk {
  readonly type = 'chunk' as const;
  #chunk: BindingRenderedChunk;

  constructor(chunk: BindingRenderedChunk) {
    super();
    this.#chunk = chunk;
  }

  @lazyProp
  get name(): string {
    return this.#chunk.name;
  }

  @lazyProp
  get isEntry(): boolean {
    return this.#chunk.isEntry;
  }

  @lazyProp
  get isDynamicEntry(): boolean {
    return this.#chunk.isDynamicEntry;
  }

  @lazyProp
  get facadeModuleId(): string | null {
    return this.#chunk.facadeModuleId;
  }

  @lazyProp
  get moduleIds(): string[] {
    return this.#chunk.moduleIds;
  }

  @lazyProp
  get exports(): string[] {
    return this.#chunk.exports;
  }

  @lazyProp
  get fileName(): string {
    return this.#chunk.fileName;
  }

  @lazyProp
  get imports(): string[] {
    return this.#chunk.imports;
  }

  @lazyProp
  get dynamicImports(): string[] {
    return this.#chunk.dynamicImports;
  }

  @lazyProp
  get modules(): { [id: string]: RenderedModule } {
    return transformChunkModules(this.#chunk.modules);
  }
}

export function transformRenderedChunk(chunk: BindingRenderedChunk): RenderedChunk {
  return new RenderedChunkImpl(chunk);
}

export function transformChunkModules(
  modules: BindingRenderedChunk['modules'],
): RenderedChunk['modules'] {
  const result: RenderedChunk['modules'] = {};
  for (let i = 0; i < modules.values.length; i++) {
    let key = modules.keys[i];
    const mod = modules.values[i];
    result[key] = transformToRenderedModule(mod);
  }
  return result;
}
