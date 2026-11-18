import { wrap } from './a.js';
export var value = wrap();
export const later = () => import('./z-b.js');
export const later2 = () => import('./zz-c.js');

export default {
  async fetch() {
    return (await import('./server.js')).server_exports;
  },
};
