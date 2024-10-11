# Diff
## /out/entry.js
### esbuild
```js
// require("./src/**/*.json") in entry.js
var globRequire_src_json = __glob({});

// import("./src/**/*.json") in entry.js
var globImport_src_json = __glob({});

// entry.js
var ab = Math.random() < 0.5 ? "a.js" : "b.js";
console.log({
  concat: {
    require: globRequire_src_json("./src/" + ab + ".json"),
    import: globImport_src_json("./src/" + ab + ".json")
  },
  template: {
    require: globRequire_src_json(`./src/${ab}.json`),
    import: globImport_src_json(`./src/${ab}.json`)
  }
});
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,13 +0,0 @@
-var globRequire_src_json = __glob({});
-var globImport_src_json = __glob({});
-var ab = Math.random() < 0.5 ? "a.js" : "b.js";
-console.log({
-    concat: {
-        require: globRequire_src_json("./src/" + ab + ".json"),
-        import: globImport_src_json("./src/" + ab + ".json")
-    },
-    template: {
-        require: globRequire_src_json(`./src/${ab}.json`),
-        import: globImport_src_json(`./src/${ab}.json`)
-    }
-});

```