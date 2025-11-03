import type { OutputChunk as RolldownOutputChunk } from 'rolldown';
import { defineTest } from 'rolldown-tests';
import { expect } from 'vitest';

export default defineTest({
  config: {
    input: ['main.js', 'test.js'],
    output: {
      entryFileNames: '[name]-entry.[ext]',
      chunkFileNames: '[name]-chunk.[ext]',
      cssEntryFileNames: '[name]-entry.[ext]',
      cssChunkFileNames: '[name]-chunk.[ext]',
    },
  },
  afterTest: (output) => {
    // Test that [ext] placeholder is replaced with 'js' for JavaScript files
    const jsFiles = output.output.filter(chunk => chunk.fileName.endsWith('.js'));
    expect(jsFiles.length).toBeGreaterThanOrEqual(2);
    expect(jsFiles.some(chunk => chunk.fileName === 'main-entry.js')).toBe(true);
    expect(jsFiles.some(chunk => chunk.fileName === 'test-entry.js')).toBe(true);

    // Test that [ext] placeholder is replaced with 'css' for CSS files
    const cssFiles = output.output.filter(chunk => chunk.fileName.endsWith('.css'));
    expect(cssFiles.some(chunk => chunk.fileName.includes('entry.css') || chunk.fileName.includes('chunk.css'))).toBe(true);
    
    // Verify no [ext] placeholders remain in any file
    output.output.forEach(chunk => {
      expect(chunk.fileName).not.toContain('[ext]');
    });
  },
});
