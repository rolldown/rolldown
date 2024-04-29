import type { OutputAsset, OutputChunk } from '../rollup'
import type {
  HasProperty,
  IsPropertiesEqual,
  TypeAssert,
} from '../utils/type-assert'
import type { RenderedModule } from './rendered-module'

export interface RolldownOutputAsset {
  type: 'asset'
  fileName: string
  source: string | Uint8Array
}

function _assertRolldownOutputAsset() {
  type _ = TypeAssert<IsPropertiesEqual<RolldownOutputAsset, OutputAsset>>
}

export interface SourceMap {
  file: string
  mappings: string
  names: string[]
  sources: string[]
  sourcesContent: string[]
  version: number
  // toString(): string
  // toUrl(): string
}

export interface RolldownOutputChunk {
  type: 'chunk'
  code: string
  isEntry: boolean
  exports: string[]
  fileName: string
  modules: {
    [id: string]: RenderedModule
  }
  imports: string[]
  dynamicImports: string[]
  facadeModuleId: string | null
  isDynamicEntry: boolean
  moduleIds: string[]
  map: SourceMap | null
  sourcemapFileName: string | null
}

function _assertRolldownOutputChunk() {
  type _ = TypeAssert<
    IsPropertiesEqual<Omit<RolldownOutputChunk, 'modules' | 'map'>, OutputChunk>
  >
}

export interface RolldownOutput {
  output: [
    RolldownOutputChunk,
    ...(RolldownOutputChunk | RolldownOutputAsset)[],
  ]
}

function _assertRolldownOutput() {
  type _ = TypeAssert<HasProperty<RolldownOutput, 'output'>>
}
