# Diff
## /out/a.js
### esbuild
```js
// a.js
var ns = __toESM(require("external"));
console.log(ns[foo](), new ns[foo]());
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var ns = __toESM(require("external"));
-console.log(ns[foo](), new ns[foo]());

```
## /out/b.js
### esbuild
```js
// b.js
var ns = __toESM(require("external"));
console.log(ns.foo(), new ns.foo());
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var ns = __toESM(require("external"));
-console.log(ns.foo(), new ns.foo());

```
## /out/c.js
### esbuild
```js
// c.js
var import_external = __toESM(require("external"));
console.log((0, import_external.default)(), (0, import_external.foo)());
console.log(new import_external.default(), new import_external.foo());
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/c.js
+++ rolldown	
@@ -1,3 +0,0 @@
-var import_external = __toESM(require("external"));
-console.log((0, import_external.default)(), (0, import_external.foo)());
-console.log(new import_external.default(), new import_external.foo());

```