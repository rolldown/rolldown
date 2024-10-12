# Diff
## /out.js
### esbuild
```js
(() => {
  // entry.js
  function e(n = arguments) {
    let t;
  }
  function u(n = arguments) {
    let t;
  }
  function a(n = arguments) {
    let t;
  }
  e();
  u();
  a();
})();
```
### rolldown
```js

//#region entry.js
function a(x = arguments) {
	let arguments$1;
}
function b(x = arguments) {
	let arguments$1;
}
function c(x = arguments) {
	let arguments$1;
}
a();
b();
c();

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,14 +1,12 @@
-(() => {
-    function e(n = arguments) {
-        let t;
-    }
-    function u(n = arguments) {
-        let t;
-    }
-    function a(n = arguments) {
-        let t;
-    }
-    e();
-    u();
-    a();
-})();
+function a(x = arguments) {
+    let arguments$1;
+}
+function b(x = arguments) {
+    let arguments$1;
+}
+function c(x = arguments) {
+    let arguments$1;
+}
+a();
+b();
+c();

```