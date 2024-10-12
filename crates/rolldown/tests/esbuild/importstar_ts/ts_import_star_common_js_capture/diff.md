# Diff
## /out.js
### esbuild
```js
// foo.ts
var require_foo = __commonJS({
  "foo.ts"(exports) {
    exports.foo = 123;
  }
});

// entry.ts
var ns = __toESM(require_foo());
var foo2 = 234;
console.log(ns, ns.foo, foo2);
```
### rolldown
```js


//#region foo.ts
var require_foo = __commonJS({ "foo.ts"(exports) {
	exports.foo = 123;
} });

//#endregion
//#region entry.ts
var import_foo = __toESM(require_foo());
let foo = 234;
console.log(import_foo, import_foo.foo, foo);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -2,7 +2,7 @@
     "foo.ts"(exports) {
         exports.foo = 123;
     }
 });
-var ns = __toESM(require_foo());
-var foo2 = 234;
-console.log(ns, ns.foo, foo2);
+var import_foo = __toESM(require_foo());
+var foo = 234;
+console.log(import_foo, import_foo.foo, foo);

```