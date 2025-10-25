import type { BindingRenderedChunk } from '../binding';
import type { AssetSource } from '../utils/asset-source';
import type { ExternalMemoryHandle } from './external-memory-handle';

export interface OutputAsset extends ExternalMemoryHandle {
  type: 'asset';
  fileName: string;
  /** @deprecated Use "originalFileNames" instead. */
  originalFileName: string | null;
  originalFileNames: string[];
  source: AssetSource;
  /** @deprecated Use "names" instead. */
  name: string | undefined;
  names: string[];
}

export interface SourceMap {
  file: string;
  mappings: string;
  names: string[];
  sources: string[];
  sourcesContent: string[];
  version: number;
  debugId?: string;
  x_google_ignoreList?: number[];
  toString(): string;
  toUrl(): string;
}

export interface RenderedModule {
  readonly code: string | null;
  renderedLength: number;
  renderedExports: string[];
}

export interface RenderedChunk extends Omit<BindingRenderedChunk, 'modules'> {
  type: 'chunk';
  modules: {
    [id: string]: RenderedModule;
  };
  name: string;
  isEntry: boolean;
  isDynamicEntry: boolean;
  facadeModuleId: string | null;
  moduleIds: Array<string>;
  exports: Array<string>;
  fileName: string;
  imports: Array<string>;
  dynamicImports: Array<string>;
}

export interface OutputChunk extends ExternalMemoryHandle {
  type: 'chunk';
  code: string;
  name: string;
  isEntry: boolean;
  exports: string[];
  fileName: string;
  modules: {
    [id: string]: RenderedModule;
  };
  imports: string[];
  dynamicImports: string[];
  facadeModuleId: string | null;
  isDynamicEntry: boolean;
  moduleIds: string[];
  map: SourceMap | null;
  sourcemapFileName: string | null;
  preliminaryFileName: string;
}

export interface RolldownOutput {
  output: [OutputChunk, ...(OutputChunk | OutputAsset)[]];
}
