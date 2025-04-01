# Reason
1. different chunk naming style
# Diff
## /out/a.js
### esbuild
```js
import {
  setValue
} from "./chunk-3GNPIT25.js";

// a.js
setValue(123);
```
### rolldown
```js
import { setValue } from "./shared.js";

//#region a.js
setValue(123);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	a.js
@@ -1,2 +1,2 @@
-import {setValue} from "./chunk-3GNPIT25.js";
+import {setValue} from "./shared.js";
 setValue(123);

```
## /out/b.js
### esbuild
```js
import "./chunk-3GNPIT25.js";
```
### rolldown
```js
import "./shared.js";

```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	b.js
@@ -1,1 +1,1 @@
-import "./chunk-3GNPIT25.js";
+import "./shared.js";

```