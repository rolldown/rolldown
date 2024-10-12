# Diff
## /out/entry.js
### esbuild
```js
import a from "pkg";
import b from "./file";
console.log(
  a,
  b,
  require("pkg2"),
  require("./file2"),
  import("./dynamic")
);
let exported;
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
-import a from "pkg";
-import b from "./file";
-console.log(a, b, require("pkg2"), require("./file2"), import("./dynamic"));
-let exported;

```
## /out/entry.css
### esbuild
```js
@import "pkg";
@import "./file";
a {
  background: url(pkg2);
}
a {
  background: url(./file2);
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.css
+++ rolldown	
@@ -1,8 +0,0 @@
-@import "pkg";
-@import "./file";
-a {
-  background: url(pkg2);
-}
-a {
-  background: url(./file2);
-}
\ No newline at end of file

```
## metafile.json
### esbuild
```js
{
  "inputs": {
    "project/entry.js": {
      "bytes": 191,
      "imports": [],
      "format": "esm"
    },
    "project/entry.css": {
      "bytes": 112,
      "imports": []
    }
  },
  "outputs": {
    "out/entry.js": {
      "imports": [
        {
          "path": "pkg",
          "kind": "import-statement",
          "external": true
        },
        {
          "path": "./file",
          "kind": "import-statement",
          "external": true
        },
        {
          "path": "pkg2",
          "kind": "require-call",
          "external": true
        },
        {
          "path": "./file2",
          "kind": "require-call",
          "external": true
        },
        {
          "path": "./dynamic",
          "kind": "dynamic-import",
          "external": true
        }
      ],
      "exports": [
        "exported"
      ],
      "entryPoint": "project/entry.js",
      "inputs": {
        "project/entry.js": {
          "bytesInOutput": 148
        }
      },
      "bytes": 148
    },
    "out/entry.css": {
      "imports": [
        {
          "path": "pkg",
          "kind": "import-rule",
          "external": true
        },
        {
          "path": "./file",
          "kind": "import-rule",
          "external": true
        },
        {
          "path": "pkg2",
          "kind": "url-token",
          "external": true
        },
        {
          "path": "./file2",
          "kind": "url-token",
          "external": true
        }
      ],
      "entryPoint": "project/entry.css",
      "inputs": {
        "project/entry.css": {
          "bytesInOutput": 65
        }
      },
      "bytes": 98
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
@@ -1,85 +0,0 @@
-{
-  "inputs": {
-    "project/entry.js": {
-      "bytes": 191,
-      "imports": [],
-      "format": "esm"
-    },
-    "project/entry.css": {
-      "bytes": 112,
-      "imports": []
-    }
-  },
-  "outputs": {
-    "out/entry.js": {
-      "imports": [
-        {
-          "path": "pkg",
-          "kind": "import-statement",
-          "external": true
-        },
-        {
-          "path": "./file",
-          "kind": "import-statement",
-          "external": true
-        },
-        {
-          "path": "pkg2",
-          "kind": "require-call",
-          "external": true
-        },
-        {
-          "path": "./file2",
-          "kind": "require-call",
-          "external": true
-        },
-        {
-          "path": "./dynamic",
-          "kind": "dynamic-import",
-          "external": true
-        }
-      ],
-      "exports": [
-        "exported"
-      ],
-      "entryPoint": "project/entry.js",
-      "inputs": {
-        "project/entry.js": {
-          "bytesInOutput": 148
-        }
-      },
-      "bytes": 148
-    },
-    "out/entry.css": {
-      "imports": [
-        {
-          "path": "pkg",
-          "kind": "import-rule",
-          "external": true
-        },
-        {
-          "path": "./file",
-          "kind": "import-rule",
-          "external": true
-        },
-        {
-          "path": "pkg2",
-          "kind": "url-token",
-          "external": true
-        },
-        {
-          "path": "./file2",
-          "kind": "url-token",
-          "external": true
-        }
-      ],
-      "entryPoint": "project/entry.css",
-      "inputs": {
-        "project/entry.css": {
-          "bytesInOutput": 65
-        }
-      },
-      "bytes": 98
-    }
-  }
-}
\ No newline at end of file

```