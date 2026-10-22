globalThis.log = [];
const target = {
  set x(v) {
    globalThis.log.push(v);
  },
  DEAD_UNUSED: 1,
};
const other = { y: 0 };
export function write(useTarget) {
  (useTarget ? target : other).x = 'written';
}
