# Reason
1. not support const enum inline
2. enum tree shaking
# Diff
## /out/simple-member.js
### esbuild
```js
// simple-member.ts
console.log(123 /* y */);
```
### rolldown
```js

//#region simple-member.ts
var x = /* @__PURE__ */ function(x) {
	x[x["y"] = 123] = "y";
	return x;
}(x || {});
console.log(x.y);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/simple-member.js
+++ rolldown	simple-member.js
@@ -1,1 +1,5 @@
-console.log(123);
+var x = (function (x) {
+    x[x["y"] = 123] = "y";
+    return x;
+})(x || ({}));
+console.log(x.y);

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

//#region simple-enum.ts
var x = /* @__PURE__ */ function(x) {
	x[x["y"] = 123] = "y";
	return x;
}(x || {});
console.log(x);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/simple-enum.js
+++ rolldown	simple-enum.js
@@ -1,5 +1,5 @@
-var x = (x2 => {
-    x2[x2["y"] = 123] = "y";
-    return x2;
+var x = (function (x) {
+    x[x["y"] = 123] = "y";
+    return x;
 })(x || ({}));
 console.log(x);

```
## /out/sibling-member.js
### esbuild
```js
// sibling-member.ts
console.log(123 /* y */, 246 /* z */);
```
### rolldown
```js

//#region sibling-member.ts
var x = /* @__PURE__ */ function(x) {
	x[x["y"] = 123] = "y";
	return x;
}(x || {});
x = /* @__PURE__ */ function(x) {
	x[x["z"] = 246] = "z";
	return x;
}(x || {});
console.log(x.y, x.z);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/sibling-member.js
+++ rolldown	sibling-member.js
@@ -1,1 +1,9 @@
-console.log(123, 246);
+var x = (function (x) {
+    x[x["y"] = 123] = "y";
+    return x;
+})(x || ({}));
+x = (function (x) {
+    x[x["z"] = 246] = "z";
+    return x;
+})(x || ({}));
+console.log(x.y, x.z);

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

//#region sibling-enum-before.ts
console.log(x);
var x = /* @__PURE__ */ function(x) {
	x[x["y"] = 123] = "y";
	return x;
}(x || {});
x = /* @__PURE__ */ function(x) {
	x[x["z"] = 246] = "z";
	return x;
}(x || {});

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/sibling-enum-before.js
+++ rolldown	sibling-enum-before.js
@@ -1,9 +1,9 @@
 console.log(x);
-var x = (x2 => {
-    x2[x2["y"] = 123] = "y";
-    return x2;
+var x = (function (x) {
+    x[x["y"] = 123] = "y";
+    return x;
 })(x || ({}));
-var x = (x2 => {
-    x2[x2["z"] = 246] = "z";
-    return x2;
+x = (function (x) {
+    x[x["z"] = 246] = "z";
+    return x;
 })(x || ({}));

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

//#region sibling-enum-middle.ts
var x = /* @__PURE__ */ function(x) {
	x[x["y"] = 123] = "y";
	return x;
}(x || {});
console.log(x);
x = /* @__PURE__ */ function(x) {
	x[x["z"] = 246] = "z";
	return x;
}(x || {});

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/sibling-enum-middle.js
+++ rolldown	sibling-enum-middle.js
@@ -1,9 +1,9 @@
-var x = (x2 => {
-    x2[x2["y"] = 123] = "y";
-    return x2;
+var x = (function (x) {
+    x[x["y"] = 123] = "y";
+    return x;
 })(x || ({}));
 console.log(x);
-var x = (x2 => {
-    x2[x2["z"] = 246] = "z";
-    return x2;
+x = (function (x) {
+    x[x["z"] = 246] = "z";
+    return x;
 })(x || ({}));

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

//#region sibling-enum-after.ts
var x = /* @__PURE__ */ function(x) {
	x[x["y"] = 123] = "y";
	return x;
}(x || {});
x = /* @__PURE__ */ function(x) {
	x[x["z"] = 246] = "z";
	return x;
}(x || {});
console.log(x);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/sibling-enum-after.js
+++ rolldown	sibling-enum-after.js
@@ -1,9 +1,9 @@
-var x = (x2 => {
-    x2[x2["y"] = 123] = "y";
-    return x2;
+var x = (function (x) {
+    x[x["y"] = 123] = "y";
+    return x;
 })(x || ({}));
-var x = (x2 => {
-    x2[x2["z"] = 246] = "z";
-    return x2;
+x = (function (x) {
+    x[x["z"] = 246] = "z";
+    return x;
 })(x || ({}));
 console.log(x);

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

//#region namespace-before.ts
let x;
(function(_x) {
	console.log(x, y);
})(x || (x = {}));
var x = /* @__PURE__ */ function(x) {
	x[x["y"] = 123] = "y";
	return x;
}(x || {});

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/namespace-before.js
+++ rolldown	namespace-before.js
@@ -1,7 +1,12 @@
-(x2 => {
-    console.log(x2, y);
+
+//#region namespace-before.ts
+let x;
+(function(_x) {
+	console.log(x, y);
 })(x || (x = {}));
-var x = (x2 => {
-    x2[x2["y"] = 123] = "y";
-    return x2;
-})(x || ({}));
+var x = /* @__PURE__ */ function(x) {
+	x[x["y"] = 123] = "y";
+	return x;
+}(x || {});
+
+//#endregion
\ No newline at end of file

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

//#region namespace-after.ts
var x = /* @__PURE__ */ function(x) {
	x[x["y"] = 123] = "y";
	return x;
}(x || {});
(function(_x) {
	console.log(x, y);
})(x || (x = {}));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/namespace-after.js
+++ rolldown	namespace-after.js
@@ -1,7 +1,7 @@
-var x = (x2 => {
-    x2[x2["y"] = 123] = "y";
-    return x2;
+var x = (function (x) {
+    x[x["y"] = 123] = "y";
+    return x;
 })(x || ({}));
-(x2 => {
-    console.log(x2, y);
+(function (_x) {
+    console.log(x, y);
 })(x || (x = {}));

```