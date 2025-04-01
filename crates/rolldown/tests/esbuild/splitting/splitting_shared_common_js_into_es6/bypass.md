# Diff
## /out/a.js
### esbuild
```js
import {
  require_shared
} from "./chunk-JQJBVS2P.js";

// a.js
var { foo } = require_shared();
console.log(foo);
```
### rolldown
```js
import { require_shared } from "./shared.js";

//#region a.js
const { foo } = require_shared();
console.log(foo);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	a.js
@@ -1,3 +1,3 @@
-import {require_shared} from "./chunk-JQJBVS2P.js";
+import {require_shared} from "./shared.js";
 var {foo} = require_shared();
 console.log(foo);

```
## /out/b.js
### esbuild
```js
import {
  require_shared
} from "./chunk-JQJBVS2P.js";

// b.js
var { foo } = require_shared();
console.log(foo);
```
### rolldown
```js
import { require_shared } from "./shared.js";

//#region b.js
const { foo } = require_shared();
console.log(foo);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	b.js
@@ -1,3 +1,3 @@
-import {require_shared} from "./chunk-JQJBVS2P.js";
+import {require_shared} from "./shared.js";
 var {foo} = require_shared();
 console.log(foo);

```
## /out/chunk-JQJBVS2P.js
### esbuild
```js
// shared.js
var require_shared = __commonJS({
  "shared.js"(exports) {
    exports.foo = 123;
  }
});

export {
  require_shared
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-JQJBVS2P.js
+++ rolldown	
@@ -1,6 +0,0 @@
-var require_shared = __commonJS({
-    "shared.js"(exports) {
-        exports.foo = 123;
-    }
-});
-export {require_shared};

```