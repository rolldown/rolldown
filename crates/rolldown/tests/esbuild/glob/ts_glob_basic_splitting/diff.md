# Reason
1. not support glob
# Diff
## /out/entry.js
### esbuild
```js
import {
  require_a
} from "./chunk-YMCIDKCT.js";
import {
  require_b
} from "./chunk-2BST4PYI.js";
import {
  __glob
} from "./chunk-WCFE7E2E.js";

// require("./src/**/*") in entry.ts
var globRequire_src = __glob({
  "./src/a.ts": () => require_a(),
  "./src/b.ts": () => require_b()
});

// import("./src/**/*") in entry.ts
var globImport_src = __glob({
  "./src/a.ts": () => import("./a-YXM4MR7E.js"),
  "./src/b.ts": () => import("./b-IPMBSSGN.js")
});

// entry.ts
var ab = Math.random() < 0.5 ? "a.ts" : "b.ts";
console.log({
  concat: {
    require: globRequire_src("./src/" + ab),
    import: globImport_src("./src/" + ab)
  },
  template: {
    require: globRequire_src(`./src/${ab}`),
    import: globImport_src(`./src/${ab}`)
  }
});
```
### rolldown
```js



//#region entry.ts
const ab = Math.random() < .5 ? "a.ts" : "b.ts";
console.log({
	concat: {
		require: __require("./src/" + ab),
		import: import("./src/" + ab)
	},
	template: {
		require: __require(`./src/${ab}`),
		import: import(`./src/${ab}`)
	}
});
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,22 +1,11 @@
-import {require_a} from "./chunk-YMCIDKCT.js";
-import {require_b} from "./chunk-2BST4PYI.js";
-import {__glob} from "./chunk-WCFE7E2E.js";
-var globRequire_src = __glob({
-    "./src/a.ts": () => require_a(),
-    "./src/b.ts": () => require_b()
-});
-var globImport_src = __glob({
-    "./src/a.ts": () => import("./a-YXM4MR7E.js"),
-    "./src/b.ts": () => import("./b-IPMBSSGN.js")
-});
-var ab = Math.random() < 0.5 ? "a.ts" : "b.ts";
+var ab = Math.random() < .5 ? "a.ts" : "b.ts";
 console.log({
     concat: {
-        require: globRequire_src("./src/" + ab),
-        import: globImport_src("./src/" + ab)
+        require: __require("./src/" + ab),
+        import: import("./src/" + ab)
     },
     template: {
-        require: globRequire_src(`./src/${ab}`),
-        import: globImport_src(`./src/${ab}`)
+        require: __require(`./src/${ab}`),
+        import: import(`./src/${ab}`)
     }
 });

```
## /out/a-YXM4MR7E.js
### esbuild
```js
import {
  require_a
} from "./chunk-YMCIDKCT.js";
import "./chunk-WCFE7E2E.js";
export default require_a();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/a-YXM4MR7E.js
+++ rolldown	
@@ -1,3 +0,0 @@
-import {require_a} from "./chunk-YMCIDKCT.js";
-import "./chunk-WCFE7E2E.js";
-export default require_a();

```
## /out/chunk-YMCIDKCT.js
### esbuild
```js
import {
  __commonJS
} from "./chunk-WCFE7E2E.js";

// src/a.ts
var require_a = __commonJS({
  "src/a.ts"(exports, module) {
    module.exports = "a";
  }
});

export {
  require_a
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-YMCIDKCT.js
+++ rolldown	
@@ -1,7 +0,0 @@
-import {__commonJS} from "./chunk-WCFE7E2E.js";
-var require_a = __commonJS({
-    "src/a.ts"(exports, module) {
-        module.exports = "a";
-    }
-});
-export {require_a};

```
## /out/b-IPMBSSGN.js
### esbuild
```js
import {
  require_b
} from "./chunk-2BST4PYI.js";
import "./chunk-WCFE7E2E.js";
export default require_b();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/b-IPMBSSGN.js
+++ rolldown	
@@ -1,3 +0,0 @@
-import {require_b} from "./chunk-2BST4PYI.js";
-import "./chunk-WCFE7E2E.js";
-export default require_b();

```
## /out/chunk-2BST4PYI.js
### esbuild
```js
import {
  __commonJS
} from "./chunk-WCFE7E2E.js";

// src/b.ts
var require_b = __commonJS({
  "src/b.ts"(exports, module) {
    module.exports = "b";
  }
});

export {
  require_b
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-2BST4PYI.js
+++ rolldown	
@@ -1,7 +0,0 @@
-import {__commonJS} from "./chunk-WCFE7E2E.js";
-var require_b = __commonJS({
-    "src/b.ts"(exports, module) {
-        module.exports = "b";
-    }
-});
-export {require_b};

```
## /out/chunk-WCFE7E2E.js
### esbuild
```js
export {
  __glob,
  __commonJS
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-WCFE7E2E.js
+++ rolldown	
@@ -1,4 +0,0 @@
-export {
-  __glob,
-  __commonJS
-};
\ No newline at end of file

```