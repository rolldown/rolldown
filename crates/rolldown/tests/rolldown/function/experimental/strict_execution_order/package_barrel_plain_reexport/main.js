(globalThis.__events ??= []).push('main');

// The page (and the reader that consumes the pure re-export barrel) lives behind a dynamic import,
// so the barrel and its side-effect-free definers end up inside an order-wrapped dynamic chunk.
export function loadPage() {
  return import('./dynamic-page.js');
}
