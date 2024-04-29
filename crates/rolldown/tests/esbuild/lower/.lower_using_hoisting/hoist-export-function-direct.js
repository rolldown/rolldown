using a = b
export function foo1() { return [a, c] }
export function bar1() { return [a, c, bar1] }
function foo2() { return [a, c] }
function bar2() { return [a, c, bar2] }
using c = d