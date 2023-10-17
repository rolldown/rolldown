import { c as color, s as size } from './chunks/shared.js';

CSS.paintWorklet.addModule(new URL('chunks/worklet.js', import.meta.url).href);

document.body.innerHTML += `<h1 style="background-image: paint(vertical-lines);">color: ${color}, size: ${size}</h1>`;
