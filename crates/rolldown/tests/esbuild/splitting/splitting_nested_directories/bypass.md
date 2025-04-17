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
import { shared_default } from "./shared.js";

//#region pageA/page.js
console.log(shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out/pageA/page.js
+++ rolldown	pageA_page.js
@@ -1,2 +1,2 @@
-import {shared_default} from "../chunk-GWC2ABNX.js";
+import {shared_default} from "./shared.js";
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
import { shared_default } from "./shared.js";

//#region pageB/page.js
console.log(-shared_default);

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out/pageB/page.js
+++ rolldown	pageB_page.js
@@ -1,2 +1,2 @@
-import {shared_default} from "../chunk-GWC2ABNX.js";
+import {shared_default} from "./shared.js";
 console.log(-shared_default);

```