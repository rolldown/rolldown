# Reason
1. the extra `let` binding could be stripped by minifier, the trivial diff caused by `oxc-transformer` followed babel behavior
# Diff
## /out/let.js
### esbuild
```js
export var x;
((x2) => {
  x2.y = 123;
})(x || (x = {}));
((x2) => {
  x2.z = x2.y;
})(x || (x = {}));
```
### rolldown
```js

//#region let.ts
let x;
(function(_x) {
	let y$1 = _x.y = 123;
})(x || (x = {}));
(function(_x2) {
	let z = _x2.z = y;
})(x || (x = {}));
//#endregion

export { x };
```
### diff
```diff
===================================================================
--- esbuild	/out/let.js
+++ rolldown	let.js
@@ -1,7 +1,8 @@
-export var x;
-(x2 => {
-    x2.y = 123;
+var x;
+(function (_x) {
+    let y$1 = _x.y = 123;
 })(x || (x = {}));
-(x2 => {
-    x2.z = x2.y;
+(function (_x2) {
+    let z = _x2.z = y;
 })(x || (x = {}));
+export {x};

```
## /out/function.js
### esbuild
```js
export var x;
((x2) => {
  function y() {
  }
  x2.y = y;
})(x || (x = {}));
((x2) => {
  x2.z = x2.y;
})(x || (x = {}));
```
### rolldown
```js

//#region function.ts
let x;
(function(_x) {
	function y$1() {}
	_x.y = y$1;
})(x || (x = {}));
(function(_x2) {
	let z = _x2.z = y;
})(x || (x = {}));
//#endregion

export { x };
```
### diff
```diff
===================================================================
--- esbuild	/out/function.js
+++ rolldown	function.js
@@ -1,8 +1,9 @@
-export var x;
-(x2 => {
-    function y() {}
-    x2.y = y;
+var x;
+(function (_x) {
+    function y$1() {}
+    _x.y = y$1;
 })(x || (x = {}));
-(x2 => {
-    x2.z = x2.y;
+(function (_x2) {
+    let z = _x2.z = y;
 })(x || (x = {}));
+export {x};

```
## /out/class.js
### esbuild
```js
export var x;
((x2) => {
  class y {
  }
  x2.y = y;
})(x || (x = {}));
((x2) => {
  x2.z = x2.y;
})(x || (x = {}));
```
### rolldown
```js

//#region class.ts
let x;
(function(_x) {
	class y$1 {}
	_x.y = y$1;
})(x || (x = {}));
(function(_x2) {
	let z = _x2.z = y;
})(x || (x = {}));
//#endregion

export { x };
```
### diff
```diff
===================================================================
--- esbuild	/out/class.js
+++ rolldown	class.js
@@ -1,8 +1,9 @@
-export var x;
-(x2 => {
-    class y {}
-    x2.y = y;
+var x;
+(function (_x) {
+    class y$1 {}
+    _x.y = y$1;
 })(x || (x = {}));
-(x2 => {
-    x2.z = x2.y;
+(function (_x2) {
+    let z = _x2.z = y;
 })(x || (x = {}));
+export {x};

```
## /out/namespace.js
### esbuild
```js
export var x;
((x2) => {
  let y;
  ((y2) => {
    0;
  })(y = x2.y || (x2.y = {}));
})(x || (x = {}));
((x2) => {
  x2.z = x2.y;
})(x || (x = {}));
```
### rolldown
```js

//#region namespace.ts
let x;
(function(_x) {
	let y$1;
	(function(_y) {})(y$1 || (y$1 = _x.y || (_x.y = {})));
})(x || (x = {}));
(function(_x2) {
	let z = _x2.z = y;
})(x || (x = {}));
//#endregion

export { x };
```
### diff
```diff
===================================================================
--- esbuild	/out/namespace.js
+++ rolldown	namespace.js
@@ -1,10 +1,9 @@
-export var x;
-(x2 => {
-    let y;
-    (y2 => {
-        0;
-    })(y = x2.y || (x2.y = {}));
+var x;
+(function (_x) {
+    let y$1;
+    (function (_y) {})(y$1 || (y$1 = _x.y || (_x.y = {})));
 })(x || (x = {}));
-(x2 => {
-    x2.z = x2.y;
+(function (_x2) {
+    let z = _x2.z = y;
 })(x || (x = {}));
+export {x};

```
## /out/enum.js
### esbuild
```js
export var x;
((x2) => {
  let y;
  ((y2) => {
  })(y = x2.y || (x2.y = {}));
})(x || (x = {}));
((x2) => {
  x2.z = x2.y;
})(x || (x = {}));
```
### rolldown
```js

//#region enum.ts
let x;
(function(_x) {
	let y$1 = /* @__PURE__ */ function(y$2) {
		return y$2;
	}({});
	_x.y = y$1;
})(x || (x = {}));
(function(_x2) {
	let z = _x2.z = y;
})(x || (x = {}));
//#endregion

export { x };
```
### diff
```diff
===================================================================
--- esbuild	/out/enum.js
+++ rolldown	enum.js
@@ -1,8 +1,11 @@
-export var x;
-(x2 => {
-    let y;
-    (y2 => {})(y = x2.y || (x2.y = {}));
+var x;
+(function (_x) {
+    let y$1 = (function (y$2) {
+        return y$2;
+    })({});
+    _x.y = y$1;
 })(x || (x = {}));
-(x2 => {
-    x2.z = x2.y;
+(function (_x2) {
+    let z = _x2.z = y;
 })(x || (x = {}));
+export {x};

```