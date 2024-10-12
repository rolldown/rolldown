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

```
### diff
```diff
===================================================================
--- esbuild	/out/let.js
+++ rolldown	
@@ -1,7 +0,0 @@
-export var x;
-(x2 => {
-    x2.y = 123;
-})(x || (x = {}));
-(x2 => {
-    x2.z = x2.y;
-})(x || (x = {}));

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

```
### diff
```diff
===================================================================
--- esbuild	/out/function.js
+++ rolldown	
@@ -1,8 +0,0 @@
-export var x;
-(x2 => {
-    function y() {}
-    x2.y = y;
-})(x || (x = {}));
-(x2 => {
-    x2.z = x2.y;
-})(x || (x = {}));

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

```
### diff
```diff
===================================================================
--- esbuild	/out/class.js
+++ rolldown	
@@ -1,8 +0,0 @@
-export var x;
-(x2 => {
-    class y {}
-    x2.y = y;
-})(x || (x = {}));
-(x2 => {
-    x2.z = x2.y;
-})(x || (x = {}));

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

```
### diff
```diff
===================================================================
--- esbuild	/out/namespace.js
+++ rolldown	
@@ -1,10 +0,0 @@
-export var x;
-(x2 => {
-    let y;
-    (y2 => {
-        0;
-    })(y = x2.y || (x2.y = {}));
-})(x || (x = {}));
-(x2 => {
-    x2.z = x2.y;
-})(x || (x = {}));

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

```
### diff
```diff
===================================================================
--- esbuild	/out/enum.js
+++ rolldown	
@@ -1,8 +0,0 @@
-export var x;
-(x2 => {
-    let y;
-    (y2 => {})(y = x2.y || (x2.y = {}));
-})(x || (x = {}));
-(x2 => {
-    x2.z = x2.y;
-})(x || (x = {}));

```