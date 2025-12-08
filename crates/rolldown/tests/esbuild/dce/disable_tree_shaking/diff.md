## /out.js
### esbuild
```js
// keep-me/index.js
console.log("side effects");

// entry.jsx
function KeepMe1() {
}
var keepMe2 = React.createElement(KeepMe1, null);
function keepMe3() {
  console.log("side effects");
}
var keepMe4 = keepMe3();
var keepMe5 = pure();
var keepMe6 = some.fn();
```
### rolldown
```js
//#region entry.jsx
function KeepMe1() {}
/* @__PURE__ */ React.createElement(KeepMe1, null);
function keepMe3() {
	console.log("side effects");
}
/* @__PURE__ */ keepMe3();
pure();
some.fn();

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,9 +1,8 @@
-console.log("side effects");
 function KeepMe1() {}
-var keepMe2 = React.createElement(KeepMe1, null);
+React.createElement(KeepMe1, null);
 function keepMe3() {
     console.log("side effects");
 }
-var keepMe4 = keepMe3();
-var keepMe5 = pure();
-var keepMe6 = some.fn();
+keepMe3();
+pure();
+some.fn();

```