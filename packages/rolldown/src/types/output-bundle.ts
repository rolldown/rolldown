import type {
  RolldownOutputAsset,
  RolldownOutputChunk,
} from './rolldown-output'

export interface OutputBundle {
  [fileName: string]: RolldownOutputAsset | RolldownOutputChunk
}
