# Reason
1. could be done in minifier
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
(function() {


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

})();
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,14 +1,14 @@
-(() => {
-    function e(n = arguments) {
-        let t;
+(function () {
+    function a(x = arguments) {
+        let arguments$1;
     }
-    function u(n = arguments) {
-        let t;
+    function b(x = arguments) {
+        let arguments$1;
     }
-    function a(n = arguments) {
-        let t;
+    function c(x = arguments) {
+        let arguments$1;
     }
-    e();
-    u();
     a();
+    b();
+    c();
 })();

```