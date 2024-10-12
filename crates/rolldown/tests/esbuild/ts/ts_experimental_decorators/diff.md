# Diff
## /out.js
### esbuild
```js
// all.ts
var Foo = class {
  constructor(arg0, arg1) {
    this.mDef = 1;
  }
  method(arg0, arg1) {
    return new Foo();
  }
  static sMethod(arg0, arg1) {
    return new Foo();
  }
};
Foo.sDef = new Foo();
__decorateClass([
  x,
  y
], Foo.prototype, "mUndef", 2);
__decorateClass([
  x,
  y
], Foo.prototype, "mDef", 2);
__decorateClass([
  x,
  y,
  __decorateParam(0, x0),
  __decorateParam(0, y0),
  __decorateParam(1, x1),
  __decorateParam(1, y1)
], Foo.prototype, "method", 1);
__decorateClass([
  x,
  y
], Foo.prototype, "mDecl", 2);
__decorateClass([
  x,
  y
], Foo.prototype, "mAbst", 2);
__decorateClass([
  x,
  y
], Foo, "sUndef", 2);
__decorateClass([
  x,
  y
], Foo, "sDef", 2);
__decorateClass([
  x,
  y,
  __decorateParam(0, x0),
  __decorateParam(0, y0),
  __decorateParam(1, x1),
  __decorateParam(1, y1)
], Foo, "sMethod", 1);
__decorateClass([
  x,
  y
], Foo, "mDecl", 2);
Foo = __decorateClass([
  x.y(),
  new y.x(),
  __decorateParam(0, x0),
  __decorateParam(0, y0),
  __decorateParam(1, x1),
  __decorateParam(1, y1)
], Foo);

// all_computed.ts
var _a, _b, _c, _d, _e, _f, _g, _h, _i, _j, _k;
var Foo2 = class {
  constructor() {
    this[_j] = 1;
    this[_f] = 2;
  }
  [(_k = mUndef(), _j = mDef(), _i = method())](arg0, arg1) {
    return new Foo2();
  }
  static [(_h = mDecl(), _g = mAbst(), xUndef(), _f = xDef(), yUndef(), _e = yDef(), _d = sUndef(), _c = sDef(), _b = sMethod(), _a = mDecl(), _b)](arg0, arg1) {
    return new Foo2();
  }
};
Foo2[_e] = 3;
Foo2[_c] = new Foo2();
__decorateClass([
  x,
  y
], Foo2.prototype, _k, 2);
__decorateClass([
  x,
  y
], Foo2.prototype, _j, 2);
__decorateClass([
  x,
  y,
  __decorateParam(0, x0),
  __decorateParam(0, y0),
  __decorateParam(1, x1),
  __decorateParam(1, y1)
], Foo2.prototype, _i, 1);
__decorateClass([
  x,
  y
], Foo2.prototype, _h, 2);
__decorateClass([
  x,
  y
], Foo2.prototype, _g, 2);
__decorateClass([
  x,
  y
], Foo2, _d, 2);
__decorateClass([
  x,
  y
], Foo2, _c, 2);
__decorateClass([
  x,
  y,
  __decorateParam(0, x0),
  __decorateParam(0, y0),
  __decorateParam(1, x1),
  __decorateParam(1, y1)
], Foo2, _b, 1);
__decorateClass([
  x,
  y
], Foo2, _a, 2);
Foo2 = __decorateClass([
  x?.[_ + "y"](),
  new y?.[_ + "x"]()
], Foo2);

// a.ts
var a_class = class {
  fn() {
    return new a_class();
  }
};
a_class.z = new a_class();
a_class = __decorateClass([
  x(() => 0),
  y(() => 1)
], a_class);
var a = a_class;

// b.ts
var b_class = class {
  fn() {
    return new b_class();
  }
};
b_class.z = new b_class();
b_class = __decorateClass([
  x(() => 0),
  y(() => 1)
], b_class);
var b = b_class;

// c.ts
var c = class {
  fn() {
    return new c();
  }
};
c.z = new c();
c = __decorateClass([
  x(() => 0),
  y(() => 1)
], c);

// d.ts
var d = class {
  fn() {
    return new d();
  }
};
d.z = new d();
d = __decorateClass([
  x(() => 0),
  y(() => 1)
], d);

// e.ts
var e_default = class {
};
e_default = __decorateClass([
  x(() => 0),
  y(() => 1)
], e_default);

// f.ts
var f = class {
  fn() {
    return new f();
  }
};
f.z = new f();
f = __decorateClass([
  x(() => 0),
  y(() => 1)
], f);

// g.ts
var g_default = class {
};
g_default = __decorateClass([
  x(() => 0),
  y(() => 1)
], g_default);

// h.ts
var h = class {
  fn() {
    return new h();
  }
};
h.z = new h();
h = __decorateClass([
  x(() => 0),
  y(() => 1)
], h);

// i.ts
var i_class = class {
};
__decorateClass([
  x(() => 0),
  y(() => 1)
], i_class.prototype, "foo", 2);
var i = i_class;

// j.ts
var j = class {
  foo() {
  }
};
__decorateClass([
  x(() => 0),
  y(() => 1)
], j.prototype, "foo", 1);

// k.ts
var k_default = class {
  foo(x2) {
  }
};
__decorateClass([
  __decorateParam(0, x(() => 0)),
  __decorateParam(0, y(() => 1))
], k_default.prototype, "foo", 1);

// arguments.ts
function dec(x2) {
}
function fn(x2) {
  var _a2;
  class Foo3 {
    [_a2 = arguments[0]]() {
    }
  }
  __decorateClass([
    dec(arguments[0])
  ], Foo3.prototype, _a2, 1);
  return Foo3;
}

// entry.js
console.log(Foo, Foo2, a, b, c, d, e_default, f, g_default, h, i, j, k_default, fn);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,267 +0,0 @@
-// all.ts
-var Foo = class {
-  constructor(arg0, arg1) {
-    this.mDef = 1;
-  }
-  method(arg0, arg1) {
-    return new Foo();
-  }
-  static sMethod(arg0, arg1) {
-    return new Foo();
-  }
-};
-Foo.sDef = new Foo();
-__decorateClass([
-  x,
-  y
-], Foo.prototype, "mUndef", 2);
-__decorateClass([
-  x,
-  y
-], Foo.prototype, "mDef", 2);
-__decorateClass([
-  x,
-  y,
-  __decorateParam(0, x0),
-  __decorateParam(0, y0),
-  __decorateParam(1, x1),
-  __decorateParam(1, y1)
-], Foo.prototype, "method", 1);
-__decorateClass([
-  x,
-  y
-], Foo.prototype, "mDecl", 2);
-__decorateClass([
-  x,
-  y
-], Foo.prototype, "mAbst", 2);
-__decorateClass([
-  x,
-  y
-], Foo, "sUndef", 2);
-__decorateClass([
-  x,
-  y
-], Foo, "sDef", 2);
-__decorateClass([
-  x,
-  y,
-  __decorateParam(0, x0),
-  __decorateParam(0, y0),
-  __decorateParam(1, x1),
-  __decorateParam(1, y1)
-], Foo, "sMethod", 1);
-__decorateClass([
-  x,
-  y
-], Foo, "mDecl", 2);
-Foo = __decorateClass([
-  x.y(),
-  new y.x(),
-  __decorateParam(0, x0),
-  __decorateParam(0, y0),
-  __decorateParam(1, x1),
-  __decorateParam(1, y1)
-], Foo);
-
-// all_computed.ts
-var _a, _b, _c, _d, _e, _f, _g, _h, _i, _j, _k;
-var Foo2 = class {
-  constructor() {
-    this[_j] = 1;
-    this[_f] = 2;
-  }
-  [(_k = mUndef(), _j = mDef(), _i = method())](arg0, arg1) {
-    return new Foo2();
-  }
-  static [(_h = mDecl(), _g = mAbst(), xUndef(), _f = xDef(), yUndef(), _e = yDef(), _d = sUndef(), _c = sDef(), _b = sMethod(), _a = mDecl(), _b)](arg0, arg1) {
-    return new Foo2();
-  }
-};
-Foo2[_e] = 3;
-Foo2[_c] = new Foo2();
-__decorateClass([
-  x,
-  y
-], Foo2.prototype, _k, 2);
-__decorateClass([
-  x,
-  y
-], Foo2.prototype, _j, 2);
-__decorateClass([
-  x,
-  y,
-  __decorateParam(0, x0),
-  __decorateParam(0, y0),
-  __decorateParam(1, x1),
-  __decorateParam(1, y1)
-], Foo2.prototype, _i, 1);
-__decorateClass([
-  x,
-  y
-], Foo2.prototype, _h, 2);
-__decorateClass([
-  x,
-  y
-], Foo2.prototype, _g, 2);
-__decorateClass([
-  x,
-  y
-], Foo2, _d, 2);
-__decorateClass([
-  x,
-  y
-], Foo2, _c, 2);
-__decorateClass([
-  x,
-  y,
-  __decorateParam(0, x0),
-  __decorateParam(0, y0),
-  __decorateParam(1, x1),
-  __decorateParam(1, y1)
-], Foo2, _b, 1);
-__decorateClass([
-  x,
-  y
-], Foo2, _a, 2);
-Foo2 = __decorateClass([
-  x?.[_ + "y"](),
-  new y?.[_ + "x"]()
-], Foo2);
-
-// a.ts
-var a_class = class {
-  fn() {
-    return new a_class();
-  }
-};
-a_class.z = new a_class();
-a_class = __decorateClass([
-  x(() => 0),
-  y(() => 1)
-], a_class);
-var a = a_class;
-
-// b.ts
-var b_class = class {
-  fn() {
-    return new b_class();
-  }
-};
-b_class.z = new b_class();
-b_class = __decorateClass([
-  x(() => 0),
-  y(() => 1)
-], b_class);
-var b = b_class;
-
-// c.ts
-var c = class {
-  fn() {
-    return new c();
-  }
-};
-c.z = new c();
-c = __decorateClass([
-  x(() => 0),
-  y(() => 1)
-], c);
-
-// d.ts
-var d = class {
-  fn() {
-    return new d();
-  }
-};
-d.z = new d();
-d = __decorateClass([
-  x(() => 0),
-  y(() => 1)
-], d);
-
-// e.ts
-var e_default = class {
-};
-e_default = __decorateClass([
-  x(() => 0),
-  y(() => 1)
-], e_default);
-
-// f.ts
-var f = class {
-  fn() {
-    return new f();
-  }
-};
-f.z = new f();
-f = __decorateClass([
-  x(() => 0),
-  y(() => 1)
-], f);
-
-// g.ts
-var g_default = class {
-};
-g_default = __decorateClass([
-  x(() => 0),
-  y(() => 1)
-], g_default);
-
-// h.ts
-var h = class {
-  fn() {
-    return new h();
-  }
-};
-h.z = new h();
-h = __decorateClass([
-  x(() => 0),
-  y(() => 1)
-], h);
-
-// i.ts
-var i_class = class {
-};
-__decorateClass([
-  x(() => 0),
-  y(() => 1)
-], i_class.prototype, "foo", 2);
-var i = i_class;
-
-// j.ts
-var j = class {
-  foo() {
-  }
-};
-__decorateClass([
-  x(() => 0),
-  y(() => 1)
-], j.prototype, "foo", 1);
-
-// k.ts
-var k_default = class {
-  foo(x2) {
-  }
-};
-__decorateClass([
-  __decorateParam(0, x(() => 0)),
-  __decorateParam(0, y(() => 1))
-], k_default.prototype, "foo", 1);
-
-// arguments.ts
-function dec(x2) {
-}
-function fn(x2) {
-  var _a2;
-  class Foo3 {
-    [_a2 = arguments[0]]() {
-    }
-  }
-  __decorateClass([
-    dec(arguments[0])
-  ], Foo3.prototype, _a2, 1);
-  return Foo3;
-}
-
-// entry.js
-console.log(Foo, Foo2, a, b, c, d, e_default, f, g_default, h, i, j, k_default, fn);
\ No newline at end of file

```