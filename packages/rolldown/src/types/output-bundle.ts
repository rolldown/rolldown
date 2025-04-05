import type { OutputAsset, OutputChunk } from './rolldown-output';

export interface OutputBundle {
  [fileName: string]: OutputAsset | OutputChunk;
}
