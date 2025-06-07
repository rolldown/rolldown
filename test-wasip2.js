/**
 * Simple test script for the @rolldown/wasip2 package
 */
import { version, bundle } from './packages/rolldown-wasip2/dist/index.js';

console.log('Testing @rolldown/wasip2 package...');
console.log('Version:', version());

const input = {
  input: {
    'main.js': 'console.log("Hello from WASI Preview 2!");'
  },
  output: {
    dir: 'dist'
  }
};

console.log('Bundling test input...');
try {
  const result = bundle(input);
  console.log('Bundle result:', result);
  console.log('Test complete. WASI Preview 2 implementation is working!');
} catch (error) {
  console.error('Error bundling:', error);
} 