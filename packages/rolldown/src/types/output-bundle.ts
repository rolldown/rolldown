import type { OutputAsset, OutputChunk } from './rolldown-output';

/** @category Plugin APIs */
export interface OutputBundle {
  [fileName: string]: OutputAsset | OutputChunk;
}
