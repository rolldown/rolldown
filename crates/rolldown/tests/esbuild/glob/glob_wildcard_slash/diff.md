# Diff
## /out.js
### esbuild
```js
// src/file-a.js
var require_file_a = __commonJS({
  "src/file-a.js"(exports, module) {
    module.exports = "a";
  }
});

// src/file-b.js
var require_file_b = __commonJS({
  "src/file-b.js"(exports, module) {
    module.exports = "b";
  }
});

// src/nested/dir/file-a.js
var require_file_a2 = __commonJS({
  "src/nested/dir/file-a.js"(exports, module) {
    module.exports = "a";
  }
});

// src/nested/dir/file-b.js
var require_file_b2 = __commonJS({
  "src/nested/dir/file-b.js"(exports, module) {
    module.exports = "b";
  }
});

// require("./src/**/*.js") in entry.js
var globRequire_src_js = __glob({
  "./src/file-a.js": () => require_file_a(),
  "./src/file-b.js": () => require_file_b(),
  "./src/nested/dir/file-a.js": () => require_file_a2(),
  "./src/nested/dir/file-b.js": () => require_file_b2()
});

// import("./src/**/*.js") in entry.js
var globImport_src_js = __glob({
  "./src/file-a.js": () => Promise.resolve().then(() => __toESM(require_file_a())),
  "./src/file-b.js": () => Promise.resolve().then(() => __toESM(require_file_b())),
  "./src/nested/dir/file-a.js": () => Promise.resolve().then(() => __toESM(require_file_a2())),
  "./src/nested/dir/file-b.js": () => Promise.resolve().then(() => __toESM(require_file_b2()))
});

// entry.js
var ab = Math.random() < 0.5 ? "a.js" : "b.js";
console.log({
  concat: {
    require: globRequire_src_js("./src/" + ab + ".js"),
    import: globImport_src_js("./src/" + ab + ".js")
  },
  template: {
    require: globRequire_src_js(`./src/${ab}.js`),
    import: globImport_src_js(`./src/${ab}.js`)
  }
});
```
### rolldown
```js

//#region entry.js
const ab = Math.random() < 0.5 ? "a.js" : "b.js";
console.log({
	concat: {
		require: require("./src/" + ab + ".js"),
		import: import("./src/" + ab + ".js")
	},
	template: {
		require: require(`./src/${ab}.js`),
		import: import(`./src/${ab}.js`)
	}
});

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,43 +1,11 @@
-var require_file_a = __commonJS({
-    "src/file-a.js"(exports, module) {
-        module.exports = "a";
-    }
-});
-var require_file_b = __commonJS({
-    "src/file-b.js"(exports, module) {
-        module.exports = "b";
-    }
-});
-var require_file_a2 = __commonJS({
-    "src/nested/dir/file-a.js"(exports, module) {
-        module.exports = "a";
-    }
-});
-var require_file_b2 = __commonJS({
-    "src/nested/dir/file-b.js"(exports, module) {
-        module.exports = "b";
-    }
-});
-var globRequire_src_js = __glob({
-    "./src/file-a.js": () => require_file_a(),
-    "./src/file-b.js": () => require_file_b(),
-    "./src/nested/dir/file-a.js": () => require_file_a2(),
-    "./src/nested/dir/file-b.js": () => require_file_b2()
-});
-var globImport_src_js = __glob({
-    "./src/file-a.js": () => Promise.resolve().then(() => __toESM(require_file_a())),
-    "./src/file-b.js": () => Promise.resolve().then(() => __toESM(require_file_b())),
-    "./src/nested/dir/file-a.js": () => Promise.resolve().then(() => __toESM(require_file_a2())),
-    "./src/nested/dir/file-b.js": () => Promise.resolve().then(() => __toESM(require_file_b2()))
-});
 var ab = Math.random() < 0.5 ? "a.js" : "b.js";
 console.log({
     concat: {
-        require: globRequire_src_js("./src/" + ab + ".js"),
-        import: globImport_src_js("./src/" + ab + ".js")
+        require: require("./src/" + ab + ".js"),
+        import: import("./src/" + ab + ".js")
     },
     template: {
-        require: globRequire_src_js(`./src/${ab}.js`),
-        import: globImport_src_js(`./src/${ab}.js`)
+        require: require(`./src/${ab}.js`),
+        import: import(`./src/${ab}.js`)
     }
 });

```