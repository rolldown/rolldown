# Diff
## /out.js
### esbuild
```js
export default function(x) {
  var _a;
  x.a;
  (_a = x.a) == null ? void 0 : _a.call(x);
  x == null ? void 0 : x.a;
  x == null ? void 0 : x.a();
  x == null ? void 0 : x.a.b;
  x == null ? void 0 : x.a.b();
  x == null ? void 0 : x["foo_"].b;
  x == null ? void 0 : x.a["bar_"];
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
@@ -1,11 +1,11 @@
-export default function (x) {
-    var _a;
-    x.a;
-    (_a = x.a) == null ? void 0 : _a.call(x);
-    x == null ? void 0 : x.a;
-    x == null ? void 0 : x.a();
-    x == null ? void 0 : x.a.b;
-    x == null ? void 0 : x.a.b();
-    x == null ? void 0 : x["foo_"].b;
-    x == null ? void 0 : x.a["bar_"];
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