# Reason
1. rolldown don't support keep name, it is part of minifier
# Diff
## /out.js
### esbuild
```js
(() => {
  function f() {
  }
  __name(f, "f"), firstImportantSideEffect(void 0);
})(), (() => {
  function g() {
  }
  __name(g, "g");
  debugger;
  secondImportantSideEffect(void 0);
})();
```
### rolldown
```js
//#region entry.js
(() => {
	function f() {}
	firstImportantSideEffect(f());
})();
(() => {
	function g() {}
	debugger;
	secondImportantSideEffect(g());
})();

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,9 +1,9 @@
-((() => {
+(() => {
     function f() {}
-    (__name(f, "f"), firstImportantSideEffect(void 0));
-})(), (() => {
+    firstImportantSideEffect(f());
+})();
+(() => {
     function g() {}
-    __name(g, "g");
     debugger;
-    secondImportantSideEffect(void 0);
-})());
+    secondImportantSideEffect(g());
+})();

```