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
import * as ns from "external";

//#region a.js
console.log(ns[foo](), new ns[foo]());

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	a.js
@@ -1,2 +1,2 @@
-var ns = __toESM(require("external"));
+import * as ns from "external";
 console.log(ns[foo](), new ns[foo]());

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
import * as ns from "external";

//#region b.js
console.log(ns.foo(), new ns.foo());

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	b.js
@@ -1,2 +1,2 @@
-var ns = __toESM(require("external"));
+import * as ns from "external";
 console.log(ns.foo(), new ns.foo());

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
import { default as def, foo } from "external";

//#region c.js
console.log(def(), foo());
console.log(new def(), new foo());

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/c.js
+++ rolldown	c.js
@@ -1,3 +1,3 @@
-var import_external = __toESM(require("external"));
-console.log((0, import_external.default)(), (0, import_external.foo)());
-console.log(new import_external.default(), new import_external.foo());
+import {default as def, foo} from "external";
+console.log(def(), foo());
+console.log(new def(), new foo());

```