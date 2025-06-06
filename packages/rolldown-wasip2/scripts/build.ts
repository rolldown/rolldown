import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.join(__dirname, '..');
const DIST = path.join(ROOT, 'dist');

// Create dist directory if it doesn't exist
if (!fs.existsSync(DIST)) {
  fs.mkdirSync(DIST, { recursive: true });
}

console.log('Building @rolldown/wasip2 package...');

// Generate the index.js file
const indexContent = `
import binding from '@rolldown/binding-wasm32-wasip2';

/**
 * Get Rolldown version 
 * @returns {string} Version string
 */
export function version() {
  return binding.version();
}

/**
 * Bundle the input files according to the given options
 * @param {object} options - Bundler options
 * @returns {object} Bundled output
 */
export function bundle(options) {
  try {
    const result = binding.bundle(typeof options === 'string' ? options : JSON.stringify(options));
    return JSON.parse(result);
  } catch (err) {
    throw new Error(\`Rolldown bundling failed: \${err.message}\`);
  }
}
`;

console.log('Writing index.js');
fs.writeFileSync(path.join(DIST, 'index.js'), indexContent);

// Generate TypeScript declarations
const dtsContent = `
/**
 * Get Rolldown version 
 */
export function version(): string;

/**
 * Bundle the input files according to the given options
 * @param options - Bundler options
 */
export function bundle(options: Record<string, any>): Record<string, any>;
`;

console.log('Writing index.d.ts');
fs.writeFileSync(path.join(DIST, 'index.d.ts'), dtsContent);

console.log('@rolldown/wasip2 package built successfully!'); 