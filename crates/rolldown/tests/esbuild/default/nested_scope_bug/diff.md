# Diff
## /out.js
### esbuild
```js
// entry.js
(() => {
  function a() {
    b();
  }
  {
    var b = () => {
    };
  }
  a();
})();
```
### rolldown
```js

//#region entry.js
(() => {
	function a() {
		b();
	}
	var b = () => {};
	a();
})();
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,9 +1,7 @@
 (() => {
     function a() {
         b();
     }
-    {
-        var b = () => {};
-    }
+    var b = () => {};
     a();
 })();

```