/* tslint:disable */
/* eslint-disable */
/**
 * @param {(FileItem)[]} file_list
 * @returns {(AssetItem)[]}
 */
export function bundle(file_list: FileItem[]): AssetItem[]
/**
 */
export class AssetItem {
  free(): void
  /**
   */
  readonly content: string
  /**
   */
  readonly name: string
}
/**
 */
export class FileItem {
  free(): void
  /**
   * @param {string} path
   * @param {string} content
   */
  constructor(path: string, content: string)
}
