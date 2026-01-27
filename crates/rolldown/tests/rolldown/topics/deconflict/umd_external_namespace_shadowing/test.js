import Quill from 'quill';
import { a, b } from 'mod';

export class Editor {
  // Parameter `quill` would shadow factory param if not renamed
  constructor(quill) {
    if (typeof Quill != 'undefined') throw new Error('Quill should not be shadowed by local quill');
    console.log(Quill, quill)
  }
}

new Editor({ default: "quill" });
