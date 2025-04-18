// Auto-generated code, DO NOT EDIT DIRECTLY!
// To edit this generated file you have to edit `tasks/generator/src/generators/hook_usage.rs`

export enum HookUsageKind {
  buildStart = 1 << 0,
  resolveId = 1 << 1,
  resolveDynamicImport = 1 << 2,
  load = 1 << 3,
  transform = 1 << 4,
  moduleParsed = 1 << 5,
  buildEnd = 1 << 6,
  renderStart = 1 << 7,
  renderError = 1 << 8,
  renderChunk = 1 << 9,
  augmentChunkHash = 1 << 10,
  generateBundle = 1 << 11,
  writeBundle = 1 << 12,
  closeBundle = 1 << 13,
  watchChange = 1 << 14,
  closeWatcher = 1 << 15,
  transformAst = 1 << 16,
  banner = 1 << 17,
  footer = 1 << 18,
  intro = 1 << 19,
  outro = 1 << 20,
}

export class HookUsage {
  constructor(public bitflag: bigint) {}

  union(kind: HookUsageKind): void {
    this.bitflag |= BigInt(kind);
  }

  // napi generate binding type `number` for `u32` in rust
  // this is only used for compatible with the behavior
  // Note: Number.MAX_SAFE_INTEGER (which is 2 ^53 - 1) so it is safe to convert bigint to number
  inner(): number {
    return Number(this.bitflag);
  }
}
