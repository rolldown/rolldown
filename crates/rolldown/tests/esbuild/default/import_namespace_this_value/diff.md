# Reason
1. rolldown split chunks
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
"use strict";
const require_chunk = require('./chunk.js');
const external = require_chunk.__toESM(require("external"));

//#region a.js
console.log(external[foo](), new external[foo]());
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	a.js
@@ -1,2 +1,3 @@
-var ns = __toESM(require("external"));
-console.log(ns[foo](), new ns[foo]());
+var require_chunk = require('./chunk.js');
+var external = require_chunk.__toESM(require("external"));
+console.log(external[foo](), new external[foo]());

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
"use strict";
const require_chunk = require('./chunk.js');
const external = require_chunk.__toESM(require("external"));

//#region b.js
console.log(external.foo(), new external.foo());
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	b.js
@@ -1,2 +1,3 @@
-var ns = __toESM(require("external"));
-console.log(ns.foo(), new ns.foo());
+var require_chunk = require('./chunk.js');
+var external = require_chunk.__toESM(require("external"));
+console.log(external.foo(), new external.foo());

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
"use strict";
const require_chunk = require('./chunk.js');
const external = require_chunk.__toESM(require("external"));

//#region c.js
console.log((0, external.default)(), (0, external.foo)());
console.log(new external.default(), new external.foo());
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/c.js
+++ rolldown	c.js
@@ -1,3 +1,4 @@
-var import_external = __toESM(require("external"));
-console.log((0, import_external.default)(), (0, import_external.foo)());
-console.log(new import_external.default(), new import_external.foo());
+var require_chunk = require('./chunk.js');
+var external = require_chunk.__toESM(require("external"));
+console.log((0, external.default)(), (0, external.foo)());
+console.log(new external.default(), new external.foo());

```