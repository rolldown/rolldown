var a = 'a'
for (var b = 'b'; 0; ) ;
if (true) { var c = 'c' }
if (true) var d = 'd'
if (false) {} else var e = 'e'
var x = 1
while (x--) var f = 'f'
do var g = 'g'; while (0);
for (; x++; ) var h = 'h'
for (var y in 'y') var i = 'i'
for (var y of 'y') var j = 'j'
export { a, b, c, d, e, f, g, h, i, j }