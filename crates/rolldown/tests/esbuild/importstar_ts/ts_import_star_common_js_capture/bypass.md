# Reason
1. sub optimal
2. could reuse `ns` binding
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
var import_foo;
var require_foo = __commonJS({ "foo.ts"(exports) {
	exports.foo = 123;
	import_foo = __toESM(require_foo());
} });

//#endregion
//#region entry.ts
require_foo();
let foo = 234;
console.log(import_foo, import_foo.foo, foo);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,10 @@
+var import_foo;
 var require_foo = __commonJS({
     "foo.ts"(exports) {
         exports.foo = 123;
+        import_foo = __toESM(require_foo());
     }
 });
-var ns = __toESM(require_foo());
-var foo2 = 234;
-console.log(ns, ns.foo, foo2);
+require_foo();
+var foo = 234;
+console.log(import_foo, import_foo.foo, foo);

```