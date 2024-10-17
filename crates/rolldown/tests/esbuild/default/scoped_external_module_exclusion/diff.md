# Reason
1. custom diff resolver
# Diff
## /out.js
### esbuild
```js
// index.js
import { Foo } from "@scope/foo";
import { Bar } from "@scope/foo/bar";
var foo = new Foo();
var bar = new Bar();
export {
  bar,
  foo
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,5 +0,0 @@
-import {Foo} from "@scope/foo";
-import {Bar} from "@scope/foo/bar";
-var foo = new Foo();
-var bar = new Bar();
-export {bar, foo};

```