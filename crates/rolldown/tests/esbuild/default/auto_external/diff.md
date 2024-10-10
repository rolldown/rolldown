# Diff
## /out/entry.js
### esbuild
```js
// entry.js
import "http://example.com/code.js";
import "https://example.com/code.js";
import "//example.com/code.js";
import "data:application/javascript;base64,ZXhwb3J0IGRlZmF1bHQgMTIz";
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,4 +0,0 @@
-import "http://example.com/code.js";
-import "https://example.com/code.js";
-import "//example.com/code.js";
-import "data:application/javascript;base64,ZXhwb3J0IGRlZmF1bHQgMTIz";

```