# Reason
1. not support const enum inline
# Diff
## /out/entry.js
### esbuild
```js
// entry.ts
function before() {
  console.log(0 /* FOO */);
}
function after() {
  console.log(0 /* FOO */);
}
export {
  after,
  before
};
```
### rolldown
```js
//#region entry.ts
function before() {
	console.log(Foo.FOO);
}
var Foo = /* @__PURE__ */ function(Foo$1) {
	Foo$1[Foo$1["FOO"] = 0] = "FOO";
	return Foo$1;
}(Foo || {});
function after() {
	console.log(Foo.FOO);
}

//#endregion
export { after, before };
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,7 +1,11 @@
 function before() {
-    console.log(0);
+    console.log(Foo.FOO);
 }
+var Foo = (function (Foo$1) {
+    Foo$1[Foo$1["FOO"] = 0] = "FOO";
+    return Foo$1;
+})(Foo || ({}));
 function after() {
-    console.log(0);
+    console.log(Foo.FOO);
 }
 export {after, before};

```