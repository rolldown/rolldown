# Diff
## /out/class.js
### esbuild
```js
// class.js
var Keep1 = class {
  *[Symbol.iterator]() {
  }
  [keep];
};
var Keep2 = class {
  [keep];
  *[Symbol.iterator]() {
  }
};
var Keep3 = class {
  *[Symbol.wtf]() {
  }
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/class.js
+++ rolldown	
@@ -1,11 +0,0 @@
-var Keep1 = class {
-    *[Symbol.iterator]() {}
-    [keep];
-};
-var Keep2 = class {
-    [keep];
-    *[Symbol.iterator]() {}
-};
-var Keep3 = class {
-    *[Symbol.wtf]() {}
-};

```
## /out/object.js
### esbuild
```js
// object.js
var keep1 = { *[Symbol.iterator]() {
}, [keep]: null };
var keep2 = { [keep]: null, *[Symbol.iterator]() {
} };
var keep3 = { *[Symbol.wtf]() {
} };
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/object.js
+++ rolldown	
@@ -1,11 +0,0 @@
-var keep1 = {
-    *[Symbol.iterator]() {},
-    [keep]: null
-};
-var keep2 = {
-    [keep]: null,
-    *[Symbol.iterator]() {}
-};
-var keep3 = {
-    *[Symbol.wtf]() {}
-};

```