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
function a(x = arguments) {}
function b(x = arguments) {}
function c(x = arguments) {}
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
@@ -1,14 +1,8 @@
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
+(function () {
+    function a(x = arguments) {}
+    function b(x = arguments) {}
+    function c(x = arguments) {}
     a();
+    b();
+    c();
 })();

```