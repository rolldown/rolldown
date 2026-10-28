import b from './b.cjs';

console.log(globalThis.__events.join(), b.seen);
