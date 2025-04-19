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
  private bitflag: bigint = BigInt(0);
  constructor() {}

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

import { Plugin } from '../..';
export function extractHookUsage(plugin: Plugin): HookUsage {
  let hookUsage = new HookUsage();

  if (plugin.buildStart) {
    hookUsage.union(HookUsageKind.buildStart);
  }

  if (plugin.resolveId) {
    hookUsage.union(HookUsageKind.resolveId);
  }

  if (plugin.resolveDynamicImport) {
    hookUsage.union(HookUsageKind.resolveDynamicImport);
  }

  if (plugin.load) {
    hookUsage.union(HookUsageKind.load);
  }

  if (plugin.transform) {
    hookUsage.union(HookUsageKind.transform);
  }

  if (plugin.moduleParsed) {
    hookUsage.union(HookUsageKind.moduleParsed);
  }

  if (plugin.buildEnd) {
    hookUsage.union(HookUsageKind.buildEnd);
  }

  if (plugin.renderStart) {
    hookUsage.union(HookUsageKind.renderStart);
  }

  if (plugin.renderError) {
    hookUsage.union(HookUsageKind.renderError);
  }

  if (plugin.renderChunk) {
    hookUsage.union(HookUsageKind.renderChunk);
  }

  if (plugin.augmentChunkHash) {
    hookUsage.union(HookUsageKind.augmentChunkHash);
  }

  if (plugin.generateBundle) {
    hookUsage.union(HookUsageKind.generateBundle);
  }

  if (plugin.writeBundle) {
    hookUsage.union(HookUsageKind.writeBundle);
  }

  if (plugin.closeBundle) {
    hookUsage.union(HookUsageKind.closeBundle);
  }

  if (plugin.watchChange) {
    hookUsage.union(HookUsageKind.watchChange);
  }

  if (plugin.closeWatcher) {
    hookUsage.union(HookUsageKind.closeWatcher);
  }

  if (plugin.banner) {
    hookUsage.union(HookUsageKind.banner);
  }

  if (plugin.footer) {
    hookUsage.union(HookUsageKind.footer);
  }

  if (plugin.intro) {
    hookUsage.union(HookUsageKind.intro);
  }

  if (plugin.outro) {
    hookUsage.union(HookUsageKind.outro);
  }

  return hookUsage;
}
