import { SafeHtml } from './safehtml.js';

export function setInnerHtml(elem, html) {
  if (html instanceof SafeHtml) {
    elem.innerHTML = html.unwrap();
  }
}
