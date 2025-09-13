import { onMount } from './a.js';

export default function _page($$payload, $$props) {
  onMount(async () => {
    console.log((await import('./b.js')).default);
  }, res());
}
