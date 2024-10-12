# Diff
## /out.js
### esbuild
```js
export default function(x) {
  x.a;
  x.a?.();
  x?.a;
  x?.a();
  x?.a.b;
  x?.a.b();
  x?.["foo_"].b;
  x?.a["bar_"];
}
```
### rolldown
```js

//#region entry.js
function entry_default(x) {
	x.foo_;
	x.foo_?.();
	x?.foo_;
	x?.foo_();
	x?.foo_.bar_;
	x?.foo_.bar_();
	x?.["foo_"].bar_;
	x?.foo_["bar_"];
}

//#endregion
export { entry_default as default };

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,10 +1,11 @@
-export default function (x) {
-    x.a;
-    x.a?.();
-    x?.a;
-    x?.a();
-    x?.a.b;
-    x?.a.b();
-    x?.["foo_"].b;
-    x?.a["bar_"];
+function entry_default(x) {
+    x.foo_;
+    x.foo_?.();
+    x?.foo_;
+    x?.foo_();
+    x?.foo_.bar_;
+    x?.foo_.bar_();
+    x?.["foo_"].bar_;
+    x?.foo_["bar_"];
 }
+export {entry_default as default};

```