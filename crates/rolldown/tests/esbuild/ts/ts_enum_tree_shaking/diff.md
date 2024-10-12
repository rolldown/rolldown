# Diff
## /out/simple-member.js
### esbuild
```js
// simple-member.ts
console.log(123 /* y */);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/simple-member.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log(123);

```
## /out/simple-enum.js
### esbuild
```js
// simple-enum.ts
var x = /* @__PURE__ */ ((x2) => {
  x2[x2["y"] = 123] = "y";
  return x2;
})(x || {});
console.log(x);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/simple-enum.js
+++ rolldown	
@@ -1,5 +0,0 @@
-var x = (x2 => {
-    x2[x2["y"] = 123] = "y";
-    return x2;
-})(x || ({}));
-console.log(x);

```
## /out/sibling-member.js
### esbuild
```js
// sibling-member.ts
console.log(123 /* y */, 246 /* z */);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/sibling-member.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log(123, 246);

```
## /out/sibling-enum-before.js
### esbuild
```js
// sibling-enum-before.ts
console.log(x);
var x = /* @__PURE__ */ ((x2) => {
  x2[x2["y"] = 123] = "y";
  return x2;
})(x || {});
var x = /* @__PURE__ */ ((x2) => {
  x2[x2["z"] = 246] = "z";
  return x2;
})(x || {});
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/sibling-enum-before.js
+++ rolldown	
@@ -1,9 +0,0 @@
-console.log(x);
-var x = (x2 => {
-    x2[x2["y"] = 123] = "y";
-    return x2;
-})(x || ({}));
-var x = (x2 => {
-    x2[x2["z"] = 246] = "z";
-    return x2;
-})(x || ({}));

```
## /out/sibling-enum-middle.js
### esbuild
```js
// sibling-enum-middle.ts
var x = /* @__PURE__ */ ((x2) => {
  x2[x2["y"] = 123] = "y";
  return x2;
})(x || {});
console.log(x);
var x = /* @__PURE__ */ ((x2) => {
  x2[x2["z"] = 246] = "z";
  return x2;
})(x || {});
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/sibling-enum-middle.js
+++ rolldown	
@@ -1,9 +0,0 @@
-var x = (x2 => {
-    x2[x2["y"] = 123] = "y";
-    return x2;
-})(x || ({}));
-console.log(x);
-var x = (x2 => {
-    x2[x2["z"] = 246] = "z";
-    return x2;
-})(x || ({}));

```
## /out/sibling-enum-after.js
### esbuild
```js
// sibling-enum-after.ts
var x = /* @__PURE__ */ ((x2) => {
  x2[x2["y"] = 123] = "y";
  return x2;
})(x || {});
var x = /* @__PURE__ */ ((x2) => {
  x2[x2["z"] = 246] = "z";
  return x2;
})(x || {});
console.log(x);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/sibling-enum-after.js
+++ rolldown	
@@ -1,9 +0,0 @@
-var x = (x2 => {
-    x2[x2["y"] = 123] = "y";
-    return x2;
-})(x || ({}));
-var x = (x2 => {
-    x2[x2["z"] = 246] = "z";
-    return x2;
-})(x || ({}));
-console.log(x);

```
## /out/namespace-before.js
### esbuild
```js
// namespace-before.ts
((x2) => {
  console.log(x2, y);
})(x || (x = {}));
var x = /* @__PURE__ */ ((x2) => {
  x2[x2["y"] = 123] = "y";
  return x2;
})(x || {});
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/namespace-before.js
+++ rolldown	
@@ -1,7 +0,0 @@
-(x2 => {
-    console.log(x2, y);
-})(x || (x = {}));
-var x = (x2 => {
-    x2[x2["y"] = 123] = "y";
-    return x2;
-})(x || ({}));

```
## /out/namespace-after.js
### esbuild
```js
// namespace-after.ts
var x = /* @__PURE__ */ ((x2) => {
  x2[x2["y"] = 123] = "y";
  return x2;
})(x || {});
((x2) => {
  console.log(x2, y);
})(x || (x = {}));
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/namespace-after.js
+++ rolldown	
@@ -1,7 +0,0 @@
-var x = (x2 => {
-    x2[x2["y"] = 123] = "y";
-    return x2;
-})(x || ({}));
-(x2 => {
-    console.log(x2, y);
-})(x || (x = {}));

```