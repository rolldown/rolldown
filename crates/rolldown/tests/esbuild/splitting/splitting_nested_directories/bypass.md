# Reason
1. different chunk naming style
# Diff
## /Users/user/project/out/pageA/page.js
### esbuild
```js
import {
  shared_default
} from "../chunk-GWC2ABNX.js";

// Users/user/project/src/pages/pageA/page.js
console.log(shared_default);
```
### rolldown
```js
import { b as shared_default } from "./shared.js";

//#region pageA/page.js
console.log(shared_default);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out/pageA/page.js
+++ rolldown	pageA_page.js
@@ -1,2 +1,2 @@
-import {shared_default} from "../chunk-GWC2ABNX.js";
+import {b as shared_default} from "./shared.js";
 console.log(shared_default);

```
## /Users/user/project/out/pageB/page.js
### esbuild
```js
import {
  shared_default
} from "../chunk-GWC2ABNX.js";

// Users/user/project/src/pages/pageB/page.js
console.log(-shared_default);
```
### rolldown
```js
import { b as shared_default } from "./shared.js";

//#region pageB/page.js
console.log(-shared_default);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out/pageB/page.js
+++ rolldown	pageB_page.js
@@ -1,2 +1,2 @@
-import {shared_default} from "../chunk-GWC2ABNX.js";
+import {b as shared_default} from "./shared.js";
 console.log(-shared_default);

```
## /Users/user/project/out/chunk-GWC2ABNX.js
### esbuild
```js
// Users/user/project/src/pages/shared.js
var shared_default = 123;

export {
  shared_default
};
```
### rolldown
```js
//#region shared.js
var shared_default = 123;

//#endregion
export { shared_default as b };
```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out/chunk-GWC2ABNX.js
+++ rolldown	shared.js
@@ -1,2 +1,2 @@
 var shared_default = 123;
-export {shared_default};
+export {shared_default as b};

```