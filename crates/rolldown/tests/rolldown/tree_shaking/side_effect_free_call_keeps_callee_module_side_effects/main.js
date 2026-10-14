import { value } from './lib/reader.js';

if (value !== 'installed') {
  throw new Error(`polyfill did not run before reader.js: ${value}`);
}
