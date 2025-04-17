# Reason
1. not support import attributes
# Diff
## /out/entry.js
### esbuild
```js
// project/data.json
var data_default = { some: "data" };

// project/data.json with { type: 'json' }
var data_default2 = { some: "data" };

// project/entry.js
x = [data_default, data_default, data_default2];
```
### rolldown
```js

//#region data.json
var some = "data";
var data_default = { some };

//#region entry.js
x = [
	data_default,
	data_default,
	data_default
];

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,7 +1,5 @@
+var some = "data";
 var data_default = {
-    some: "data"
+    some
 };
-var data_default2 = {
-    some: "data"
-};
-x = [data_default, data_default, data_default2];
+x = [data_default, data_default, data_default];

```
## metafile.json
### esbuild
```js
{
  "inputs": {
    "project/data.json": {
      "bytes": 16,
      "imports": []
    },
    "project/data.json with { type: 'json' }": {
      "bytes": 16,
      "imports": [],
      "format": "esm",
      "with": {
        "type": "json"
      }
    },
    "project/entry.js": {
      "bytes": 164,
      "imports": [
        {
          "path": "project/data.json",
          "kind": "import-statement",
          "original": "./data.json"
        },
        {
          "path": "project/data.json",
          "kind": "import-statement",
          "original": "./data.json"
        },
        {
          "path": "project/data.json with { type: 'json' }",
          "kind": "import-statement",
          "original": "./data.json",
          "with": {
            "type": "json"
          }
        }
      ],
      "format": "esm"
    }
  },
  "outputs": {
    "out/entry.js": {
      "imports": [],
      "exports": [],
      "entryPoint": "project/entry.js",
      "inputs": {
        "project/data.json": {
          "bytesInOutput": 37
        },
        "project/data.json with { type: 'json' }": {
          "bytesInOutput": 38
        },
        "project/entry.js": {
          "bytesInOutput": 49
        }
      },
      "bytes": 210
    }
  }
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	metafile.json
+++ rolldown	
@@ -1,59 +0,0 @@
-{
-  "inputs": {
-    "project/data.json": {
-      "bytes": 16,
-      "imports": []
-    },
-    "project/data.json with { type: 'json' }": {
-      "bytes": 16,
-      "imports": [],
-      "format": "esm",
-      "with": {
-        "type": "json"
-      }
-    },
-    "project/entry.js": {
-      "bytes": 164,
-      "imports": [
-        {
-          "path": "project/data.json",
-          "kind": "import-statement",
-          "original": "./data.json"
-        },
-        {
-          "path": "project/data.json",
-          "kind": "import-statement",
-          "original": "./data.json"
-        },
-        {
-          "path": "project/data.json with { type: 'json' }",
-          "kind": "import-statement",
-          "original": "./data.json",
-          "with": {
-            "type": "json"
-          }
-        }
-      ],
-      "format": "esm"
-    }
-  },
-  "outputs": {
-    "out/entry.js": {
-      "imports": [],
-      "exports": [],
-      "entryPoint": "project/entry.js",
-      "inputs": {
-        "project/data.json": {
-          "bytesInOutput": 37
-        },
-        "project/data.json with { type: 'json' }": {
-          "bytesInOutput": 38
-        },
-        "project/entry.js": {
-          "bytesInOutput": 49
-        }
-      },
-      "bytes": 210
-    }
-  }
-}
\ No newline at end of file

```