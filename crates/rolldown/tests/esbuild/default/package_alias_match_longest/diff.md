# Reason
1. alias not align
# Diff
## /out.js
### esbuild
```js
// entry.js
import "alias/pkg";
import "alias/pkg_foo";
import "alias/pkg_foo_bar";
import "alias/pkg_foo_bar/baz";
import "alias/pkg/bar/baz";
import "alias/pkg/baz";
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,6 +0,0 @@
-import "alias/pkg";
-import "alias/pkg_foo";
-import "alias/pkg_foo_bar";
-import "alias/pkg_foo_bar/baz";
-import "alias/pkg/bar/baz";
-import "alias/pkg/baz";

```