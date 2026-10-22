globalThis.log = [];
function f() {}
Object.defineProperty(f, 'x', {
  get() {
    globalThis.log.push('getter');
    return 1;
  },
});
f.x;
export const done = true;
