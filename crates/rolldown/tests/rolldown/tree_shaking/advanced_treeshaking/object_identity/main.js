globalThis.log = [];
function make() {
  return { v: 1 };
}
if (make() === make()) globalThis.log.push('equal');
else globalThis.log.push('different');
export const done = true;
