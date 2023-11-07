/* tslint:disable */
/* eslint-disable */
/**
* @param {(FileItem)[]} file_list
* @returns {(AssetItem)[]}
*/
export function bundle(file_list: (FileItem)[]): (AssetItem)[];
/**
*/
export class AssetItem {
  free(): void;
/**
*/
  readonly content: string;
/**
*/
  readonly name: string;
}
/**
*/
export class FileItem {
  free(): void;
/**
* @param {string} path
* @param {string} content
*/
  constructor(path: string, content: string);
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_fileitem_free: (a: number) => void;
  readonly fileitem_new: (a: number, b: number, c: number, d: number) => number;
  readonly __wbg_assetitem_free: (a: number) => void;
  readonly assetitem_name: (a: number, b: number) => void;
  readonly assetitem_content: (a: number, b: number) => void;
  readonly bundle: (a: number, b: number, c: number) => void;
  readonly __wbindgen_malloc: (a: number, b: number) => number;
  readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
  readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
  readonly __wbindgen_free: (a: number, b: number, c: number) => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;
/**
* Instantiates the given `module`, which can either be bytes or
* a precompiled `WebAssembly.Module`.
*
* @param {SyncInitInput} module
*
* @returns {InitOutput}
*/
export function initSync(module: SyncInitInput): InitOutput;

/**
* If `module_or_path` is {RequestInfo} or {URL}, makes a request and
* for everything else, calls `WebAssembly.instantiate` directly.
*
* @param {InitInput | Promise<InitInput>} module_or_path
*
* @returns {Promise<InitOutput>}
*/
export default function __wbg_init (module_or_path?: InitInput | Promise<InitInput>): Promise<InitOutput>;
