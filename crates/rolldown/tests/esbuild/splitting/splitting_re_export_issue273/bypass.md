# Reason
1. different chunk naming style
# Diff
## /out/a.js
### esbuild
```js
import {
  a
} from "./chunk-RLFZNZQZ.js";
export {
  a
};
```
### rolldown
```js
import { a } from "./a2.js";

export { a };
```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	a.js
@@ -1,2 +1,2 @@
-import {a} from "./chunk-RLFZNZQZ.js";
+import {a} from "./a2.js";
 export {a};

```
## /out/b.js
### esbuild
```js
import {
  a
} from "./chunk-RLFZNZQZ.js";
export {
  a
};
```
### rolldown
```js
import { a } from "./a2.js";

export { a };
```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	b.js
@@ -1,2 +1,2 @@
-import {a} from "./chunk-RLFZNZQZ.js";
+import {a} from "./a2.js";
 export {a};

```