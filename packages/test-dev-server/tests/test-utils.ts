import nodeFs from 'node:fs';
import { resolve } from 'node:path';
import { CONFIG } from './src/config.js';

const testDir = CONFIG.paths.tmpFullBundleModeDir;

/**
 * Edit a file using Node.js fs module
 * Files are edited in the tmp directory, not the original source
 */
export async function editFile(
  filename: string,
  replacer: (content: string) => string,
): Promise<void> {
  const filePath = resolve(testDir, filename);
  const content = nodeFs.readFileSync(filePath, 'utf-8');
  const newContent = replacer(content);
  if (content === newContent) {
    console.warn(`[editFile] No changes detected for ${filename}`);
    return;
  }
  nodeFs.writeFileSync(filePath, newContent, 'utf-8');

  // Small delay to ensure file system events are picked up
  await new Promise(resolve => setTimeout(resolve, 1000));

  console.log(`[editFile] Updated ${filename}`);
}

/**
 * Get the Playwright page from global context
 */
export function getPage() {
  const page = (global as any).__page;
  if (!page) {
    throw new Error(
      'Playwright page not initialized. Check vitest-setup-playwright.ts',
    );
  }
  return page;
}
