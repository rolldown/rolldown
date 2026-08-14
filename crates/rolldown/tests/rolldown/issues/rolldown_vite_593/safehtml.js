import { isIE } from './browser.js';

export class SafeHtml {
  constructor(html) {
    this.html = html;
    this.isIE = isIE();
  }

  unwrap() {
    return this.html;
  }
}
