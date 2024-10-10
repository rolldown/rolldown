using a = b
function foo1() { return [a, c] }
function bar1() { return [a, c, bar1] }
function foo2() { return [a, c] }
function bar2() { return [a, c, bar2] }
using c = d
export {foo1, bar1}