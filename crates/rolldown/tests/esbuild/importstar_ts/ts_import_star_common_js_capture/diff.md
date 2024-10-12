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

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,8 +0,0 @@
-var require_foo = __commonJS({
-    "foo.ts"(exports) {
-        exports.foo = 123;
-    }
-});
-var ns = __toESM(require_foo());
-var foo2 = 234;
-console.log(ns, ns.foo, foo2);

```