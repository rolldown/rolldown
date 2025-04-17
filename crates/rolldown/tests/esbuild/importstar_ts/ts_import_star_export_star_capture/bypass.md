# Reason
1. different deconflict naming style and order
# Diff
## /out.js
### esbuild
```js
// bar.ts
var bar_exports = {};
__export(bar_exports, {
  foo: () => foo
});

// foo.ts
var foo = 123;

// entry.ts
var foo2 = 234;
console.log(bar_exports, foo, foo2);
```
### rolldown
```js

//#region rolldown:runtime
var __defProp = Object.defineProperty;
var __export = (target, all) => {
	for (var name in all) __defProp(target, name, {
		get: all[name],
		enumerable: true
	});
};

//#region foo.ts
const foo$1 = 123;

//#region bar.ts
var bar_exports = {};
__export(bar_exports, { foo: () => foo$1 });

//#region entry.ts
let foo = 234;
console.log(bar_exports, foo$1, foo);

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,7 +1,14 @@
+var __defProp = Object.defineProperty;
+var __export = (target, all) => {
+    for (var name in all) __defProp(target, name, {
+        get: all[name],
+        enumerable: true
+    });
+};
+var foo$1 = 123;
 var bar_exports = {};
 __export(bar_exports, {
-    foo: () => foo
+    foo: () => foo$1
 });
-var foo = 123;
-var foo2 = 234;
-console.log(bar_exports, foo, foo2);
+var foo = 234;
+console.log(bar_exports, foo$1, foo);

```