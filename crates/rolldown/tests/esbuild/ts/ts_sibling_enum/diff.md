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

```
### diff
```diff
===================================================================
--- esbuild	/out/number.js
+++ rolldown	
@@ -1,13 +0,0 @@
-export var x = (x2 => {
-    x2[x2["y"] = 0] = "y";
-    x2[x2["yy"] = 0] = "yy";
-    return x2;
-})(x || ({}));
-var x = (x2 => {
-    x2[x2["z"] = 1] = "z";
-    return x2;
-})(x || ({}));
-(x2 => {
-    console.log(y, z);
-})(x || (x = {}));
-console.log(0, 1);

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

```
### diff
```diff
===================================================================
--- esbuild	/out/string.js
+++ rolldown	
@@ -1,13 +0,0 @@
-export var x = (x2 => {
-    x2["y"] = "a";
-    x2["yy"] = "a";
-    return x2;
-})(x || ({}));
-var x = (x2 => {
-    x2["z"] = "a";
-    return x2;
-})(x || ({}));
-(x2 => {
-    console.log(y, z);
-})(x || (x = {}));
-console.log("a", "a");

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

```
### diff
```diff
===================================================================
--- esbuild	/out/propagation.js
+++ rolldown	
@@ -1,16 +0,0 @@
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
-})(x || ({}));
-var x = (x2 => {
-    x2[x2["g"] = 625] = "g";
-    return x2;
-})(x || ({}));
-console.log(100, 100, 625, 625);

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

```
### diff
```diff
===================================================================
--- esbuild	/out/nested-number.js
+++ rolldown	
@@ -1,21 +0,0 @@
-export var foo;
-(foo2 => {
-    let x;
-    (x2 => {
-        x2[x2["y"] = 0] = "y";
-        x2[x2["yy"] = 0] = "yy";
-    })(x = foo2.x || (foo2.x = {}));
-})(foo || (foo = {}));
-(foo2 => {
-    let x;
-    (x2 => {
-        x2[x2["z"] = 1] = "z";
-    })(x = foo2.x || (foo2.x = {}));
-})(foo || (foo = {}));
-(foo2 => {
-    let x;
-    (x2 => {
-        console.log(y, z);
-        console.log(0, 1);
-    })(x = foo2.x || (foo2.x = {}));
-})(foo || (foo = {}));

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

```
### diff
```diff
===================================================================
--- esbuild	/out/nested-string.js
+++ rolldown	
@@ -1,21 +0,0 @@
-export var foo;
-(foo2 => {
-    let x;
-    (x2 => {
-        x2["y"] = "a";
-        x2["yy"] = "a";
-    })(x = foo2.x || (foo2.x = {}));
-})(foo || (foo = {}));
-(foo2 => {
-    let x;
-    (x2 => {
-        x2["z"] = "a";
-    })(x = foo2.x || (foo2.x = {}));
-})(foo || (foo = {}));
-(foo2 => {
-    let x;
-    (x2 => {
-        console.log(y, z);
-        console.log("a", "a");
-    })(x = foo2.x || (foo2.x = {}));
-})(foo || (foo = {}));

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

```
### diff
```diff
===================================================================
--- esbuild	/out/nested-propagation.js
+++ rolldown	
@@ -1,23 +0,0 @@
-export var n;
-(n2 => {
-    let a;
-    (a2 => {
-        a2[a2["b"] = 100] = "b";
-    })(a = n2.a || (n2.a = {}));
-})(n || (n = {}));
-(n2 => {
-    let x;
-    (x2 => {
-        x2[x2["c"] = 100] = "c";
-        x2[x2["d"] = 200] = "d";
-        x2[x2["e"] = 4e4] = "e";
-        x2[x2["f"] = 1e4] = "f";
-    })(x = n2.x || (n2.x = {}));
-})(n || (n = {}));
-(n2 => {
-    let x;
-    (x2 => {
-        x2[x2["g"] = 625] = "g";
-    })(x = n2.x || (n2.x = {}));
-    console.log(100, 100, 100, 625, 625, 625);
-})(n || (n = {}));

```