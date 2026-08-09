import type { BindingRenderedChunk } from '../binding.cjs';
import type { RenderedChunk } from '../types/rolldown-output';
import { transformToRenderedModule } from './transform-rendered-module';

export function transformRenderedChunk(chunk: BindingRenderedChunk): RenderedChunk {
  let modules: null | RenderedChunk['modules'] = null;
  return {
    type: 'chunk',
    get name() {
      return chunk.name;
    },
    get isEntry() {
      return chunk.isEntry;
    },
    get isDynamicEntry() {
      return chunk.isDynamicEntry;
    },
    get facadeModuleId() {
      return chunk.facadeModuleId;
    },
    get moduleIds() {
      return chunk.moduleIds;
    },
    get exports() {
      return chunk.exports;
    },
    get fileName() {
      return chunk.fileName;
    },
    get imports() {
      return chunk.imports;
    },
    get dynamicImports() {
      return chunk.dynamicImports;
    },
    get modules() {
      if (!modules) {
        modules = transformChunkModules(chunk.modules);
      }
      return modules;
    },
  };
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
 * data (modules included, via {@linkcode snapshotChunkModules}) and releases
 * the native `BindingRenderedChunk` box immediately. For the threadless-WASI
 * flavor, where GC finalizers (the box's normal reclamation path) never run.
 * A callback that retains the returned chunk past its hook invocation keeps
 * working: nothing on it reads through the released box.
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
 * plain JS data and releases each native `BindingRenderedModule` box
 * immediately. For the threadless-WASI flavor, where GC finalizers (the boxes'
 * normal reclamation path) never run.
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
