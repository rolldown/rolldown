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

/**
 * Like {@linkcode transformRenderedChunk}, but copies every field to plain JS
 * data (modules included) and releases the native box immediately, for the
 * threadless-WASI flavor. A callback that retains the returned chunk past its
 * hook keeps working: nothing on it reads through the released box.
 */
export function snapshotRenderedChunk(chunk: BindingRenderedChunk): RenderedChunk {
  const snapshot: RenderedChunk = {
    type: 'chunk',
    name: chunk.name,
    isEntry: chunk.isEntry,
    isDynamicEntry: chunk.isDynamicEntry,
    facadeModuleId: chunk.facadeModuleId,
    moduleIds: chunk.moduleIds,
    exports: chunk.exports,
    fileName: chunk.fileName,
    imports: chunk.imports,
    dynamicImports: chunk.dynamicImports,
    modules: snapshotChunkModules(chunk.modules),
  };
  chunk.dropInner();
  return snapshot;
}

/**
 * Like {@linkcode transformChunkModules}, but copies every rendered module to
 * plain JS data and releases each native box immediately, for the
 * threadless-WASI flavor.
 */
export function snapshotChunkModules(
  modules: BindingRenderedChunk['modules'],
): RenderedChunk['modules'] {
  const result: RenderedChunk['modules'] = {};
  for (let i = 0; i < modules.values.length; i++) {
    const key = modules.keys[i];
    const box = modules.values[i];
    const code = box.code;
    result[key] = {
      code,
      renderedLength: code?.length || 0,
      renderedExports: box.renderedExports,
    };
    box.dropInner();
  }
  return result;
}
