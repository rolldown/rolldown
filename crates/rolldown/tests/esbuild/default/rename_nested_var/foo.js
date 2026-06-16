export function foo(a, b) {}
var topLevel = 0
{ var nested = 1 }
function fn() { var inner = 2 }
foo(topLevel, nested, fn)