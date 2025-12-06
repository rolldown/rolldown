import { onMount } from './a.js';

// Test case: multiple dynamic imports in a single top-level statement
// Both import('./b.js') and import('./c.js') should be replaced with void 0
// because they are inside the callback of a side-effect-free function
export default function _page($$payload, $$props) {
  onMount(async () => {
    const b = await import('./b.js');
    const c = await import('./c.js');
    console.log(b.default, c.default);
  });
}
