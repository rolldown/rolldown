# Diff
## /out.js
### esbuild
```js
// src/a.js
var require_a = __commonJS({
  "src/a.js"(exports, module) {
    module.exports = "a";
  }
});

// src/b.js
var require_b = __commonJS({
  "src/b.js"(exports, module) {
    module.exports = "b";
  }
});

// require("./src/**/*") in entry.js
var globRequire_src = __glob({
  "./src/a.js": () => require_a(),
  "./src/b.js": () => require_b()
});

// import("./src/**/*") in entry.js
var globImport_src = __glob({
  "./src/a.js": () => Promise.resolve().then(() => __toESM(require_a())),
  "./src/b.js": () => Promise.resolve().then(() => __toESM(require_b()))
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
const ab = Math.random() < 0.5 ? "a.js" : "b.js";
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
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,29 +1,11 @@
-var require_a = __commonJS({
-    "src/a.js"(exports, module) {
-        module.exports = "a";
-    }
-});
-var require_b = __commonJS({
-    "src/b.js"(exports, module) {
-        module.exports = "b";
-    }
-});
-var globRequire_src = __glob({
-    "./src/a.js": () => require_a(),
-    "./src/b.js": () => require_b()
-});
-var globImport_src = __glob({
-    "./src/a.js": () => Promise.resolve().then(() => __toESM(require_a())),
-    "./src/b.js": () => Promise.resolve().then(() => __toESM(require_b()))
-});
 var ab = Math.random() < 0.5 ? "a.js" : "b.js";
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