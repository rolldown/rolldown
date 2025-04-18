# Reason
1. enum inline
# Diff
## /out/number.js
### esbuild
```js
export var x = /* @__PURE__ */ ((x2) => {
  x2[x2["y"] = 0] = "y";
  x2[x2["yy"] = 0 /* y */] = "yy";
  return x2;
})(x || {});
var x = /* @__PURE__ */ ((x2) => {
  x2[x2["z"] = 1] = "z";
  return x2;
})(x || {});
((x2) => {
  console.log(y, z);
})(x || (x = {}));
console.log(0 /* y */, 1 /* z */);
```
### rolldown
```js
//#region number.ts
let x = /* @__PURE__ */ function(x$1) {
	x$1[x$1["y"] = 0] = "y";
	x$1[x$1["yy"] = 0] = "yy";
	return x$1;
}({});
x = /* @__PURE__ */ function(x$1) {
	x$1[x$1["z"] = 1] = "z";
	return x$1;
}(x || {});
(function(_x) {
	console.log(y, z);
})(x || (x = {}));
console.log(x.y, x.z);

//#endregion
export { x };
```
### diff
```diff
===================================================================
--- esbuild	/out/number.js
+++ rolldown	number.js
@@ -1,13 +1,14 @@
-export var x = (x2 => {
-    x2[x2["y"] = 0] = "y";
-    x2[x2["yy"] = 0] = "yy";
-    return x2;
+var x = (function (x$1) {
+    x$1[x$1["y"] = 0] = "y";
+    x$1[x$1["yy"] = 0] = "yy";
+    return x$1;
+})({});
+x = (function (x$1) {
+    x$1[x$1["z"] = 1] = "z";
+    return x$1;
 })(x || ({}));
-var x = (x2 => {
-    x2[x2["z"] = 1] = "z";
-    return x2;
-})(x || ({}));
-(x2 => {
+(function (_x) {
     console.log(y, z);
 })(x || (x = {}));
-console.log(0, 1);
+console.log(x.y, x.z);
+export {x};

```
## /out/string.js
### esbuild
```js
export var x = /* @__PURE__ */ ((x2) => {
  x2["y"] = "a";
  x2["yy"] = "a" /* y */;
  return x2;
})(x || {});
var x = /* @__PURE__ */ ((x2) => {
  x2["z"] = "a" /* y */;
  return x2;
})(x || {});
((x2) => {
  console.log(y, z);
})(x || (x = {}));
console.log("a" /* y */, "a" /* z */);
```
### rolldown
```js
//#region string.ts
let x = /* @__PURE__ */ function(x$1) {
	x$1["y"] = "a";
	x$1["yy"] = "a";
	return x$1;
}({});
x = /* @__PURE__ */ function(x$1) {
	x$1["z"] = "a";
	return x$1;
}(x || {});
(function(_x) {
	console.log(y, z);
})(x || (x = {}));
console.log(x.y, x.z);

//#endregion
export { x };
```
### diff
```diff
===================================================================
--- esbuild	/out/string.js
+++ rolldown	string.js
@@ -1,13 +1,14 @@
-export var x = (x2 => {
-    x2["y"] = "a";
-    x2["yy"] = "a";
-    return x2;
+var x = (function (x$1) {
+    x$1["y"] = "a";
+    x$1["yy"] = "a";
+    return x$1;
+})({});
+x = (function (x$1) {
+    x$1["z"] = "a";
+    return x$1;
 })(x || ({}));
-var x = (x2 => {
-    x2["z"] = "a";
-    return x2;
-})(x || ({}));
-(x2 => {
+(function (_x) {
     console.log(y, z);
 })(x || (x = {}));
-console.log("a", "a");
+console.log(x.y, x.z);
+export {x};

```
## /out/propagation.js
### esbuild
```js
export var a = /* @__PURE__ */ ((a2) => {
  a2[a2["b"] = 100] = "b";
  return a2;
})(a || {});
export var x = /* @__PURE__ */ ((x2) => {
  x2[x2["c"] = 100 /* b */] = "c";
  x2[x2["d"] = 200] = "d";
  x2[x2["e"] = 4e4] = "e";
  x2[x2["f"] = 1e4] = "f";
  return x2;
})(x || {});
var x = /* @__PURE__ */ ((x2) => {
  x2[x2["g"] = 625] = "g";
  return x2;
})(x || {});
console.log(100 /* b */, 100 /* b */, 625 /* g */, 625 /* g */);
```
### rolldown
```js
//#region propagation.ts
let a = /* @__PURE__ */ function(a$1) {
	a$1[a$1["b"] = 100] = "b";
	return a$1;
}({});
let x = /* @__PURE__ */ function(x$1) {
	x$1[x$1["c"] = 100] = "c";
	x$1[x$1["d"] = 200] = "d";
	x$1[x$1["e"] = x$1.d ** 2] = "e";
	x$1[x$1["f"] = x$1["e"] / 4] = "f";
	return x$1;
}({});
x = /* @__PURE__ */ function(x$1) {
	x$1[x$1["g"] = x$1.f >> 4] = "g";
	return x$1;
}(x || {});
console.log(a.b, a["b"], x.g, x["g"]);

//#endregion
export { a, x };
```
### diff
```diff
===================================================================
--- esbuild	/out/propagation.js
+++ rolldown	propagation.js
@@ -1,16 +1,17 @@
-export var a = (a2 => {
-    a2[a2["b"] = 100] = "b";
-    return a2;
-})(a || ({}));
-export var x = (x2 => {
-    x2[x2["c"] = 100] = "c";
-    x2[x2["d"] = 200] = "d";
-    x2[x2["e"] = 4e4] = "e";
-    x2[x2["f"] = 1e4] = "f";
-    return x2;
+var a = (function (a$1) {
+    a$1[a$1["b"] = 100] = "b";
+    return a$1;
+})({});
+var x = (function (x$1) {
+    x$1[x$1["c"] = 100] = "c";
+    x$1[x$1["d"] = 200] = "d";
+    x$1[x$1["e"] = x$1.d ** 2] = "e";
+    x$1[x$1["f"] = x$1["e"] / 4] = "f";
+    return x$1;
+})({});
+x = (function (x$1) {
+    x$1[x$1["g"] = x$1.f >> 4] = "g";
+    return x$1;
 })(x || ({}));
-var x = (x2 => {
-    x2[x2["g"] = 625] = "g";
-    return x2;
-})(x || ({}));
-console.log(100, 100, 625, 625);
+console.log(a.b, a["b"], x.g, x["g"]);
+export {a, x};

```
## /out/nested-number.js
### esbuild
```js
export var foo;
((foo2) => {
  let x;
  ((x2) => {
    x2[x2["y"] = 0] = "y";
    x2[x2["yy"] = 0 /* y */] = "yy";
  })(x = foo2.x || (foo2.x = {}));
})(foo || (foo = {}));
((foo2) => {
  let x;
  ((x2) => {
    x2[x2["z"] = 1] = "z";
  })(x = foo2.x || (foo2.x = {}));
})(foo || (foo = {}));
((foo2) => {
  let x;
  ((x2) => {
    console.log(y, z);
    console.log(0 /* y */, 1 /* z */);
  })(x = foo2.x || (foo2.x = {}));
})(foo || (foo = {}));
```
### rolldown
```js
//#region nested-number.ts
let foo;
(function(_foo) {
	let x = /* @__PURE__ */ function(x$1) {
		x$1[x$1["y"] = 0] = "y";
		x$1[x$1["yy"] = 0] = "yy";
		return x$1;
	}({});
	_foo.x = x;
})(foo || (foo = {}));
(function(_foo2) {
	let x = /* @__PURE__ */ function(x$1) {
		x$1[x$1["z"] = 1] = "z";
		return x$1;
	}({});
	_foo2.x = x;
})(foo || (foo = {}));
(function(_foo3) {
	let x;
	(function(_x) {
		console.log(y, z);
		console.log(x.y, x.z);
	})(x || (x = _foo3.x || (_foo3.x = {})));
})(foo || (foo = {}));

//#endregion
export { foo };
```
### diff
```diff
===================================================================
--- esbuild	/out/nested-number.js
+++ rolldown	nested-number.js
@@ -1,21 +1,24 @@
-export var foo;
-(foo2 => {
-    let x;
-    (x2 => {
-        x2[x2["y"] = 0] = "y";
-        x2[x2["yy"] = 0] = "yy";
-    })(x = foo2.x || (foo2.x = {}));
+var foo;
+(function (_foo) {
+    let x = (function (x$1) {
+        x$1[x$1["y"] = 0] = "y";
+        x$1[x$1["yy"] = 0] = "yy";
+        return x$1;
+    })({});
+    _foo.x = x;
 })(foo || (foo = {}));
-(foo2 => {
-    let x;
-    (x2 => {
-        x2[x2["z"] = 1] = "z";
-    })(x = foo2.x || (foo2.x = {}));
+(function (_foo2) {
+    let x = (function (x$1) {
+        x$1[x$1["z"] = 1] = "z";
+        return x$1;
+    })({});
+    _foo2.x = x;
 })(foo || (foo = {}));
-(foo2 => {
+(function (_foo3) {
     let x;
-    (x2 => {
+    (function (_x) {
         console.log(y, z);
-        console.log(0, 1);
-    })(x = foo2.x || (foo2.x = {}));
+        console.log(x.y, x.z);
+    })(x || (x = _foo3.x || (_foo3.x = {})));
 })(foo || (foo = {}));
+export {foo};

```
## /out/nested-string.js
### esbuild
```js
export var foo;
((foo2) => {
  let x;
  ((x2) => {
    x2["y"] = "a";
    x2["yy"] = "a" /* y */;
  })(x = foo2.x || (foo2.x = {}));
})(foo || (foo = {}));
((foo2) => {
  let x;
  ((x2) => {
    x2["z"] = "a" /* y */;
  })(x = foo2.x || (foo2.x = {}));
})(foo || (foo = {}));
((foo2) => {
  let x;
  ((x2) => {
    console.log(y, z);
    console.log("a" /* y */, "a" /* z */);
  })(x = foo2.x || (foo2.x = {}));
})(foo || (foo = {}));
```
### rolldown
```js
//#region nested-string.ts
let foo;
(function(_foo) {
	let x = /* @__PURE__ */ function(x$1) {
		x$1["y"] = "a";
		x$1["yy"] = "a";
		return x$1;
	}({});
	_foo.x = x;
})(foo || (foo = {}));
(function(_foo2) {
	let x = /* @__PURE__ */ function(x$1) {
		x$1["z"] = "a";
		return x$1;
	}({});
	_foo2.x = x;
})(foo || (foo = {}));
(function(_foo3) {
	let x;
	(function(_x) {
		console.log(y, z);
		console.log(x.y, x.z);
	})(x || (x = _foo3.x || (_foo3.x = {})));
})(foo || (foo = {}));

//#endregion
export { foo };
```
### diff
```diff
===================================================================
--- esbuild	/out/nested-string.js
+++ rolldown	nested-string.js
@@ -1,21 +1,24 @@
-export var foo;
-(foo2 => {
-    let x;
-    (x2 => {
-        x2["y"] = "a";
-        x2["yy"] = "a";
-    })(x = foo2.x || (foo2.x = {}));
+var foo;
+(function (_foo) {
+    let x = (function (x$1) {
+        x$1["y"] = "a";
+        x$1["yy"] = "a";
+        return x$1;
+    })({});
+    _foo.x = x;
 })(foo || (foo = {}));
-(foo2 => {
-    let x;
-    (x2 => {
-        x2["z"] = "a";
-    })(x = foo2.x || (foo2.x = {}));
+(function (_foo2) {
+    let x = (function (x$1) {
+        x$1["z"] = "a";
+        return x$1;
+    })({});
+    _foo2.x = x;
 })(foo || (foo = {}));
-(foo2 => {
+(function (_foo3) {
     let x;
-    (x2 => {
+    (function (_x) {
         console.log(y, z);
-        console.log("a", "a");
-    })(x = foo2.x || (foo2.x = {}));
+        console.log(x.y, x.z);
+    })(x || (x = _foo3.x || (_foo3.x = {})));
 })(foo || (foo = {}));
+export {foo};

```
## /out/nested-propagation.js
### esbuild
```js
export var n;
((n2) => {
  let a;
  ((a2) => {
    a2[a2["b"] = 100] = "b";
  })(a = n2.a || (n2.a = {}));
})(n || (n = {}));
((n2) => {
  let x;
  ((x2) => {
    x2[x2["c"] = 100 /* b */] = "c";
    x2[x2["d"] = 200] = "d";
    x2[x2["e"] = 4e4] = "e";
    x2[x2["f"] = 1e4] = "f";
  })(x = n2.x || (n2.x = {}));
})(n || (n = {}));
((n2) => {
  let x;
  ((x2) => {
    x2[x2["g"] = 625] = "g";
  })(x = n2.x || (n2.x = {}));
  console.log(100 /* b */, 100 /* b */, 100 /* b */, 625 /* g */, 625 /* g */, 625 /* g */);
})(n || (n = {}));
```
### rolldown
```js
//#region nested-propagation.ts
let n;
(function(_n) {
	let a$1 = /* @__PURE__ */ function(a$2) {
		a$2[a$2["b"] = 100] = "b";
		return a$2;
	}({});
	_n.a = a$1;
})(n || (n = {}));
(function(_n2) {
	let x = /* @__PURE__ */ function(x$1) {
		x$1[x$1["c"] = n.a.b] = "c";
		x$1[x$1["d"] = x$1.c * 2] = "d";
		x$1[x$1["e"] = x$1.d ** 2] = "e";
		x$1[x$1["f"] = x$1["e"] / 4] = "f";
		return x$1;
	}({});
	_n2.x = x;
})(n || (n = {}));
(function(_n3) {
	let x = /* @__PURE__ */ function(x$1) {
		x$1[x$1["g"] = x$1.f >> 4] = "g";
		return x$1;
	}({});
	_n3.x = x;
	console.log(a.b, n.a.b, n["a"]["b"], x.g, n.x.g, n["x"]["g"]);
})(n || (n = {}));

//#endregion
export { n };
```
### diff
```diff
===================================================================
--- esbuild	/out/nested-propagation.js
+++ rolldown	nested-propagation.js
@@ -1,23 +1,27 @@
-export var n;
-(n2 => {
-    let a;
-    (a2 => {
-        a2[a2["b"] = 100] = "b";
-    })(a = n2.a || (n2.a = {}));
+var n;
+(function (_n) {
+    let a$1 = (function (a$2) {
+        a$2[a$2["b"] = 100] = "b";
+        return a$2;
+    })({});
+    _n.a = a$1;
 })(n || (n = {}));
-(n2 => {
-    let x;
-    (x2 => {
-        x2[x2["c"] = 100] = "c";
-        x2[x2["d"] = 200] = "d";
-        x2[x2["e"] = 4e4] = "e";
-        x2[x2["f"] = 1e4] = "f";
-    })(x = n2.x || (n2.x = {}));
+(function (_n2) {
+    let x = (function (x$1) {
+        x$1[x$1["c"] = n.a.b] = "c";
+        x$1[x$1["d"] = x$1.c * 2] = "d";
+        x$1[x$1["e"] = x$1.d ** 2] = "e";
+        x$1[x$1["f"] = x$1["e"] / 4] = "f";
+        return x$1;
+    })({});
+    _n2.x = x;
 })(n || (n = {}));
-(n2 => {
-    let x;
-    (x2 => {
-        x2[x2["g"] = 625] = "g";
-    })(x = n2.x || (n2.x = {}));
-    console.log(100, 100, 100, 625, 625, 625);
+(function (_n3) {
+    let x = (function (x$1) {
+        x$1[x$1["g"] = x$1.f >> 4] = "g";
+        return x$1;
+    })({});
+    _n3.x = x;
+    console.log(a.b, n.a.b, n["a"]["b"], x.g, n.x.g, n["x"]["g"]);
 })(n || (n = {}));
+export {n};

```