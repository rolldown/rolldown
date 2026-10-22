const config = { lazy: false };
export function load() {
  if (config.lazy) return import('./lazy.js');
  return null;
}
