globalThis.log = [];
function f() {}
f.__proto__ = {
  set x(value) {
    globalThis.log.push(value);
  },
};
f.x = 'set';
export const done = true;
