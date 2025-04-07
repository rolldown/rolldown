import { rolldown } from 'rolldown';

const bundle = await rolldown({
  input: ['./index.js'],
});
await bundle.write({ format: 'esm', dir: './dist' });
// Execute twice
await bundle.write({ format: 'esm', dir: './dist' });
