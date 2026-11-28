import type { OutputAsset, OutputChunk } from './rolldown-output';

type Mutable<T> = { -readonly [K in keyof T]: T[K] };

/** @category Plugin APIs */
export interface OutputBundle {
  [fileName: string]: Mutable<OutputAsset> | Mutable<OutputChunk>;
}
