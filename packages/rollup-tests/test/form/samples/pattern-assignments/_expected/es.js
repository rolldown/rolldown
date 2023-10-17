var effect = () => console.log( 'effect' );

var { a } = { a: effect };
a();

var { x: b } = { x: effect };
b();

const s = {};
var { c } = { c: s };
c.foo = 1;

const t = {};
var { x: d } = { x: t };
d.foo = 1;

var e;
({ e } = { e: effect });
e();

var f;
({ x: f } = { x: effect });
f();

const u = {};
var g;
({ g } = { g: u });
g.foo = 1;

const v = {};
var h;
({ x: h } = { x: v });
h.foo = 1;

export { s, t, u, v };
