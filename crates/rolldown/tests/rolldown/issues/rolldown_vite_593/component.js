import { htmlEscape, setInnerHtml } from './shared.js';

export class Component {
  constructor(name) {
    this.name = name;
  }

  render(el) {
    const escaped = htmlEscape(this.name);
    setInnerHtml(el, escaped);
  }
}
