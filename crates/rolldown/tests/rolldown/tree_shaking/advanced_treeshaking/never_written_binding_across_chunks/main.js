import { custom_render } from './flag.js';
import { mount as mc } from './client.js';
export function mount(...a) {
  if (custom_render) {
    return mc(...a);
  }
  throw new Error('server only');
}
