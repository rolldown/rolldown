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
import "pkg";
import "pkg/foo";
import "pkg/foo/bar";

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,3 @@
-import "alias/pkg";
-import "alias/pkg_foo";
-import "alias/pkg_foo_bar";
-import "alias/pkg_foo_bar/baz";
-import "alias/pkg/bar/baz";
-import "alias/pkg/baz";
+import "pkg";
+import "pkg/foo";
+import "pkg/foo/bar";

```