import { defineTest } from 'rolldown-tests';
import { viteImportGlobPlugin } from 'rolldown/experimental';
import { unlinkSync } from 'node:fs';
import { symlink } from 'node:fs/promises';
import { join } from 'node:path';
import { platform } from 'node:process';

const __dirname = import.meta.dirname;
// `dir/sub/loop` links back to `dir`, so a walk that follows it never ends.
const linkPath = join(__dirname, 'dir', 'sub', 'loop');
const targetPath = join(__dirname, 'dir');

function removeLink() {
  try {
    unlinkSync(linkPath);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error;
  }
}

export default defineTest({
  config: {
    plugins: [viteImportGlobPlugin()],
  },
  async beforeTest() {
    removeLink();
    // On Windows, a junction needs no administrator privileges or Developer Mode.
    await symlink(targetPath, linkPath, platform === 'win32' ? 'junction' : 'dir');
  },
  async afterTest() {
    try {
      await import('./assert.mjs');
    } finally {
      removeLink();
    }
  },
});
