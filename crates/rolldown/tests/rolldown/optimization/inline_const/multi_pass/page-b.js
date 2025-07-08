import { derived } from './foo.js';
function demo() {
  if (derived) {
    console.log('page-b');
  }
}

demo();
