import { value } from './lib/index.js';

if (value !== 'installed') {
  throw new Error(`polyfill did not run before reader.js: ${value}`);
}
