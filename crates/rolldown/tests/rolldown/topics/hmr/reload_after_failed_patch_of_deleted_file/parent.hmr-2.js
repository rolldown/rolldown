import assert from 'node:assert';

export const value = 'still-updating';
assert.strictEqual(globalThis.__reload_after_failed_patch_of_deleted_file_step, 1);
globalThis.__reload_after_failed_patch_of_deleted_file_step = 2;
import.meta.hot.accept();
