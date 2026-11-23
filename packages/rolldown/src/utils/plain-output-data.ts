import type {
  BindingAssetSource,
  BindingModules,
  BindingOutputAsset,
  BindingOutputAssetData,
  BindingOutputChunk,
  BindingOutputChunkData,
  ExternalMemoryStatus,
} from '../binding.cjs';

// A wasm binding gives outputs as plain data, because the GC does not see Wasm memory (see
// `PLAIN_DATA` in `crates/rolldown_binding`). These classes give the plain data the methods of
// the binding objects, so the rest of the code reads both the same way. Like the binding methods,
// they return a new array or object on each call, because a plugin can change a returned value
// in place and `collectChunkChange` compares it with a new one.

const FREED_ERROR =
  'Memory has been freed by `freeExternalMemory()`. Cannot access properties. To prevent this, use `freeExternalMemory(handle, true)` with `keepDataAlive`.';

function dropData(hadData: boolean): ExternalMemoryStatus {
  return hadData ? { freed: true } : { freed: false, reason: 'Memory has already been freed' };
}

// Native identities are addresses, so negative numbers never match one.
let lastCodeIdentity = 0;

class PlainOutputChunk implements BindingOutputChunk {
  #data: BindingOutputChunkData | undefined;
  readonly #codeIdentity = --lastCodeIdentity;

  constructor(data: BindingOutputChunkData) {
    this.#data = data;
  }

  get #chunk(): BindingOutputChunkData {
    if (!this.#data) throw new Error(FREED_ERROR);
    return this.#data;
  }

  dropInner(): ExternalMemoryStatus {
    const hadData = this.#data !== undefined;
    this.#data = undefined;
    return dropData(hadData);
  }

  getIsEntry(): boolean {
    return this.#chunk.isEntry;
  }

  getIsDynamicEntry(): boolean {
    return this.#chunk.isDynamicEntry;
  }

  getFacadeModuleId(): string | null {
    return this.#chunk.facadeModuleId;
  }

  getModuleIds(): Array<string> {
    return [...this.#chunk.moduleIds];
  }

  getExports(): Array<string> {
    return [...this.#chunk.exports];
  }

  getFileName(): string {
    return this.#chunk.fileName;
  }

  getModules(): BindingModules {
    const { values, keys } = this.#chunk.modules;
    return { values: [...values], keys: [...keys] };
  }

  getImports(): Array<string> {
    return [...this.#chunk.imports];
  }

  getDynamicImports(): Array<string> {
    return [...this.#chunk.dynamicImports];
  }

  getCode(): string {
    return this.#chunk.code;
  }

  getCodeIdentity(): number {
    return this.#codeIdentity;
  }

  getMap(): string | null {
    return this.#chunk.map;
  }

  getSourcemapFileName(): string | null {
    return this.#chunk.sourcemapFileName;
  }

  getPreliminaryFileName(): string {
    return this.#chunk.preliminaryFileName;
  }

  getName(): string {
    return this.#chunk.name;
  }
}

class PlainOutputAsset implements BindingOutputAsset {
  #data: BindingOutputAssetData | undefined;

  constructor(data: BindingOutputAssetData) {
    this.#data = data;
  }

  get #asset(): BindingOutputAssetData {
    if (!this.#data) throw new Error(FREED_ERROR);
    return this.#data;
  }

  dropInner(): ExternalMemoryStatus {
    const hadData = this.#data !== undefined;
    this.#data = undefined;
    return dropData(hadData);
  }

  getFileName(): string {
    return this.#asset.fileName;
  }

  getOriginalFileName(): string | null {
    return this.#asset.originalFileNames[0] ?? null;
  }

  getOriginalFileNames(): Array<string> {
    return [...this.#asset.originalFileNames];
  }

  getSource(): BindingAssetSource {
    const { inner } = this.#asset.source;
    return { inner: typeof inner === 'string' ? inner : inner.slice() };
  }

  getName(): string | null {
    return this.#asset.names[0] ?? null;
  }

  getNames(): Array<string> {
    return [...this.#asset.names];
  }
}

export function toBindingOutputChunk(
  chunk: BindingOutputChunk | BindingOutputChunkData,
): BindingOutputChunk {
  return 'code' in chunk ? new PlainOutputChunk(chunk) : chunk;
}

export function toBindingOutputAsset(
  asset: BindingOutputAsset | BindingOutputAssetData,
): BindingOutputAsset {
  return 'source' in asset ? new PlainOutputAsset(asset) : asset;
}
