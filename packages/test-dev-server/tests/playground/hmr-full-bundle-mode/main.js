import './hmr.js';
import './hmr-error/module.js';
import './rebuild-error/module.js';

text('.app', 'hello');

function text(el, text) {
  document.querySelector(el).textContent = text;
}
