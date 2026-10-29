import { decorate } from './facade.js';
import cjsFacade from './cjs-facade.cjs';
import conditionalCjsFacade from './conditional-cjs-facade.cjs';

export async function load() {
  const dynamicExisting = await import('./dynamic-existing.js');
  return [
    decorate('a'),
    cjsFacade.decorate('b'),
    conditionalCjsFacade.decorate('c'),
    dynamicExisting.value,
  ].join(' ');
}

// Accept updates here, so an edit to implementation.js does not reload the tabs
// that loaded this module.
import.meta.hot?.accept();
