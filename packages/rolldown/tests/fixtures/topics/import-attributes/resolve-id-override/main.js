import pkg from './package.json';
import ext from 'ext' with { type: 'json' };
import kept from 'kept' with { type: 'json' };

export { pkg, ext, kept };
export const lazy = () => [
  import('./package.json'),
  import('ext', { with: { type: 'json' } }),
  import('kept', { with: { type: 'json' } }),
];
