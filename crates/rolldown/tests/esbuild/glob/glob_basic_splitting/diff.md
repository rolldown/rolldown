# Reason
1. not support glob
# Diff
## /out/entry.js
### esbuild
```js
import {
  require_a
} from "./chunk-KO426RN2.js";
import {
  require_b
} from "./chunk-SGVK3D4Q.js";
import {
  __glob
} from "./chunk-WCFE7E2E.js";

// require("./src/**/*") in entry.js
var globRequire_src = __glob({
  "./src/a.js": () => require_a(),
  "./src/b.js": () => require_b()
});

// import("./src/**/*") in entry.js
var globImport_src = __glob({
  "./src/a.js": () => import("./a-7QA47R6Z.js"),
  "./src/b.js": () => import("./b-KY4MVCQS.js")
});

// entry.js
var ab = Math.random() < 0.5 ? "a.js" : "b.js";
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

//#region entry.js
const ab = Math.random() < .5 ? "a.js" : "b.js";
console.log({
	concat: {
		require: require("./src/" + ab),
		import: import("./src/" + ab)
	},
	template: {
		require: require(`./src/${ab}`),
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
-import {require_a} from "./chunk-KO426RN2.js";
-import {require_b} from "./chunk-SGVK3D4Q.js";
-import {__glob} from "./chunk-WCFE7E2E.js";
-var globRequire_src = __glob({
-    "./src/a.js": () => require_a(),
-    "./src/b.js": () => require_b()
-});
-var globImport_src = __glob({
-    "./src/a.js": () => import("./a-7QA47R6Z.js"),
-    "./src/b.js": () => import("./b-KY4MVCQS.js")
-});
-var ab = Math.random() < 0.5 ? "a.js" : "b.js";
+var ab = Math.random() < .5 ? "a.js" : "b.js";
 console.log({
     concat: {
-        require: globRequire_src("./src/" + ab),
-        import: globImport_src("./src/" + ab)
+        require: require("./src/" + ab),
+        import: import("./src/" + ab)
     },
     template: {
-        require: globRequire_src(`./src/${ab}`),
-        import: globImport_src(`./src/${ab}`)
+        require: require(`./src/${ab}`),
+        import: import(`./src/${ab}`)
     }
 });

```
## /out/a-7QA47R6Z.js
### esbuild
```js
import {
  require_a
} from "./chunk-KO426RN2.js";
import "./chunk-WCFE7E2E.js";
export default require_a();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/a-7QA47R6Z.js
+++ rolldown	
@@ -1,3 +0,0 @@
-import {require_a} from "./chunk-KO426RN2.js";
-import "./chunk-WCFE7E2E.js";
-export default require_a();

```
## /out/chunk-KO426RN2.js
### esbuild
```js
import {
  __commonJS
} from "./chunk-WCFE7E2E.js";

// src/a.js
var require_a = __commonJS({
  "src/a.js"(exports, module) {
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
--- esbuild	/out/chunk-KO426RN2.js
+++ rolldown	
@@ -1,7 +0,0 @@
-import {__commonJS} from "./chunk-WCFE7E2E.js";
-var require_a = __commonJS({
-    "src/a.js"(exports, module) {
-        module.exports = "a";
-    }
-});
-export {require_a};

```
## /out/b-KY4MVCQS.js
### esbuild
```js
import {
  require_b
} from "./chunk-SGVK3D4Q.js";
import "./chunk-WCFE7E2E.js";
export default require_b();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/b-KY4MVCQS.js
+++ rolldown	
@@ -1,3 +0,0 @@
-import {require_b} from "./chunk-SGVK3D4Q.js";
-import "./chunk-WCFE7E2E.js";
-export default require_b();

```
## /out/chunk-SGVK3D4Q.js
### esbuild
```js
import {
  __commonJS
} from "./chunk-WCFE7E2E.js";

// src/b.js
var require_b = __commonJS({
  "src/b.js"(exports, module) {
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
--- esbuild	/out/chunk-SGVK3D4Q.js
+++ rolldown	
@@ -1,7 +0,0 @@
-import {__commonJS} from "./chunk-WCFE7E2E.js";
-var require_b = __commonJS({
-    "src/b.js"(exports, module) {
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