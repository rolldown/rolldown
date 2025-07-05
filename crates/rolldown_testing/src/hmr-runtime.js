// @ts-nocheck FIXME(hyf0): Enable type check

/// <reference path="../../../crates/rolldown_plugin_hmr/src/runtime/runtime-extra-dev-common.js" />

class TestDevRuntime extends DevRuntime {
  /**
   * @override
   * @param {string} _moduleId
   */
  createModuleHotContext(_moduleId) {
    return { accept() {} };
  }
  /**
   * @override
   * @param {string[]} _boundaries
   */
  applyUpdates(_boundaries) {
    // do nothing
  }
}

(/** @type {any} */ (globalThis)).__rolldown_runtime__ ??= new TestDevRuntime();

/** @type {string[]} */
const testPatches = /** @type {any} */ (globalThis).__testPatches;
if (testPatches) {
  setTimeout(async () => {
    for (const patchChunk of testPatches) {
      await import(patchChunk);
    }
  }, 0);
}
