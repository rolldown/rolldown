import { isWasiTest } from '@tests/runtime-flavor';
import { defineTest } from 'rolldown-tests';
import { viteImportGlobPlugin } from 'rolldown/experimental';
import { existsSync, lstatSync, unlinkSync, renameSync } from 'node:fs';
import { symlink } from 'node:fs/promises';
import { join } from 'node:path';
import { platform } from 'node:process';

const __dirname = import.meta.dirname;
const linkPath = join(__dirname, 'linked', 'my-lib');
const targetPath = join(__dirname, 'packages', 'my-lib');
const backupPath = linkPath + '.bak';

export default defineTest({
  // KNOWN: wasi fs limitation. `walkdir` with `follow_links(true)` (used by
  // rolldown_plugin_vite_import_glob) cannot traverse the `linked/my-lib`
  // directory symlink on wasm32-wasip1, so the glob
  // `./linked/*/components/*.js` matches 0 of the 2 expected modules.
  skip: isWasiTest,
  config: {
    plugins: [viteImportGlobPlugin()],
  },

  // On Windows, Git for Windows defaults `core.symlinks` to `false`, so
  // symlinks tracked in the repository are checked out as plain text files
  // containing the link target path instead of real symbolic links. Since
  // `linked/my-lib` must be a directory for the glob
  // `'./linked/*/components/*.js'` to match, recreate it as a real directory
  // link below.
  async beforeTest() {
    if (existsSync(linkPath)) {
      const stat = lstatSync(linkPath);

      if (!stat.isSymbolicLink()) {
        renameSync(linkPath, backupPath);
      }
    }
    // Create a directory symlink/junction so that walkdir (with
    // `follow_links(true)`) in `rolldown_plugin_vite_import_glob`
    // can traverse into `packages/my-lib/components/`.
    // - On Windows, use `'junction'` which doesn't require administrator
    //   privileges or Developer Mode (unlike real symlinks).
    // - On Unix, use `'dir'` which creates a standard symbolic link.
    if (!existsSync(linkPath)) {
      await symlink(targetPath, linkPath, platform === 'win32' ? 'junction' : 'dir');
    }
  },
  async afterTest() {
    await import('./assert.mjs');

    if (existsSync(backupPath)) {
      if (existsSync(linkPath)) {
        unlinkSync(linkPath);
      }
      renameSync(backupPath, linkPath);
    }
  },
});
