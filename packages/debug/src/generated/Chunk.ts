// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { ChunkImport } from './ChunkImport';

export type Chunk = {
  id: number;
  /**
   * ```js
   * import { defineConfig } from 'rolldown';
   * export default defineConfig({
   *   input: {
   *     main: './index.ts',
   *   },
   *   output: {
   *     advancedChunks: {
   *       groups: [
   *         {
   *           name: 'npm-libs',
   *           test: /node_modules/,
   *         },
   *       ],
   *     },
   *   },
   * });
   * ```
   * - `main` is the name, if this chunk is an entry chunk.
   * - `npm-libs` is the name, if this chunk is created by `output.advancedChunks`.
   */
  name: string | null;
  /**
   * ```js
   * import { defineConfig } from 'rolldown';
   * export default defineConfig({
   *   input: {
   *     main: './index.ts',
   *   },
   *   output: {
   *     advancedChunks: {
   *       groups: [
   *         {
   *           name: 'npm-libs',
   *           test: /node_modules/,
   *         },
   *       ],
   *     },
   *   },
   * });
   * ```
   * - `group_index` will be `0` if this chunk is created by `output.advancedChunks`.
   */
  group_index: number | null;
  is_user_defined_entry: boolean;
  /**
   * A entry could be both user-defined and async.
   */
  is_async_entry: boolean;
  entry_module: string | null;
  modules: Array<string>;
  reason: 'advanced-chunks' | 'preserve-modules' | 'entry' | 'common';
  imports: Array<ChunkImport>;
};
