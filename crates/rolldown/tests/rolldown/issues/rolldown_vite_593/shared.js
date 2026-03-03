import { setInnerHtml } from './safe.js';

export function compareVersions(a, b) {
  return a > b ? 1 : a < b ? -1 : 0;
}

export function htmlEscape(str) {
  return str.replace(/&/g, '&amp;').replace(/</g, '&lt;');
}

export { setInnerHtml };
