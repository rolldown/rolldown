// Like @vueless/storybook-dark-mode: it uses ONLY the helper-independent
// `themes` passthrough, from a TOP-LEVEL side effect (which keeps theming.js and
// its now-dangling __commonJS / __toESM calls in the graph).
import { themes } from './theming.js';

globalThis.__themes = themes;

export function getProjectAnnotations() {
  return [globalThis.__themes];
}
