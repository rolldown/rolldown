(globalThis.__events ??= []).push('main');

// The page (and everything it reaches) lives behind a dynamic import, so the reader that consumes
// the definer ends up inside an order-wrapped dynamic chunk.
export function loadPage() {
  return import('./dynamic-page.js');
}
