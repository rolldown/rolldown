# Reason
2. not support copy loader
# Diff
## /out/bytesInOutput should be at least 99 (1).js
### esbuild
```js
// project/111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111.file
var __default = "./111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111-55DNWN2R.file";

// project/bytesInOutput should be at least 99 (1).js
console.log(__default);
```
### rolldown
```js

//#region 111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111.file
var _111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111_default = "assets/111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111-pbxtN44L.file";
//#endregion

//#region bytesInOutput should be at least 99 (1).js
console.log(_111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111_default);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/bytesInOutput should be at least 99 (1).js
+++ rolldown	bytesInOutput should be at least 99 (1).js
@@ -1,2 +1,2 @@
-var __default = "./111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111-55DNWN2R.file";
-console.log(__default);
+var _111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111_default = "assets/111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111-pbxtN44L.file";
+console.log(_111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111_default);

```
## /out/bytesInOutput should be at least 99 (2).js
### esbuild
```js
// project/bytesInOutput should be at least 99 (2).js
import a from "./222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222-55DNWN2R.copy";
console.log(a);
```
### rolldown
```js

//#region 222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222.copy
var default$1 = void 0;
//#endregion

//#region bytesInOutput should be at least 99 (2).js
console.log(default$1);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/bytesInOutput should be at least 99 (2).js
+++ rolldown	bytesInOutput should be at least 99 (2).js
@@ -1,2 +1,2 @@
-import a from "./222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222-55DNWN2R.copy";
-console.log(a);
+var default$1 = void 0;
+console.log(default$1);

```
## /out/bytesInOutput should be at least 99 (3).js
### esbuild
```js
// project/bytesInOutput should be at least 99 (3).js
import("./333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333-DH3FVEAA.js").then(console.log);
```
### rolldown
```js

//#region bytesInOutput should be at least 99 (3).js
import("./333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333.js").then(console.log);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/bytesInOutput should be at least 99 (3).js
+++ rolldown	bytesInOutput should be at least 99 (3).js
@@ -1,1 +1,1 @@
-import("./333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333-DH3FVEAA.js").then(console.log);
+import("./333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333.js").then(console.log);

```
## /out/bytesInOutput should be at least 99.css
### esbuild
```js
/* project/bytesInOutput should be at least 99.css */
a {
  background: url("./444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444-55DNWN2R.file");
}
```
### rolldown
```js
a { background: url(assets/444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444-CrPpbTAK.file) }


```
### diff
```diff
===================================================================
--- esbuild	/out/bytesInOutput should be at least 99.css
+++ rolldown	bytesInOutput should be at least 99.css
@@ -1,4 +1,2 @@
-/* project/bytesInOutput should be at least 99.css */
-a {
-  background: url("./444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444-55DNWN2R.file");
-}
\ No newline at end of file
+a { background: url(assets/444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444-CrPpbTAK.file) }
+

```
## metafile.json
### esbuild
```js
{
  "inputs": {
    "project/111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111.file": {
      "bytes": 0,
      "imports": []
    },
    "project/bytesInOutput should be at least 99 (1).js": {
      "bytes": 150,
      "imports": [
        {
          "path": "project/111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111.file",
          "kind": "import-statement",
          "original": "./111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111.file"
        }
      ],
      "format": "esm"
    },
    "project/222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222.copy": {
      "bytes": 0,
      "imports": []
    },
    "project/bytesInOutput should be at least 99 (2).js": {
      "bytes": 150,
      "imports": [
        {
          "path": "project/222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222.copy",
          "kind": "import-statement",
          "original": "./222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222.copy"
        }
      ],
      "format": "esm"
    },
    "project/333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333.js": {
      "bytes": 0,
      "imports": []
    },
    "project/bytesInOutput should be at least 99 (3).js": {
      "bytes": 141,
      "imports": [
        {
          "path": "project/333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333.js",
          "kind": "dynamic-import",
          "original": "./333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333.js"
        }
      ]
    },
    "project/444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444.file": {
      "bytes": 0,
      "imports": []
    },
    "project/bytesInOutput should be at least 99.css": {
      "bytes": 136,
      "imports": [
        {
          "path": "project/444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444.file",
          "kind": "url-token",
          "original": "444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444.file"
        }
      ]
    }
  },
  "outputs": {
    "out/111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111-55DNWN2R.file": {
      "imports": [],
      "exports": [],
      "inputs": {
        "project/111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111.file": {
          "bytesInOutput": 0
        }
      },
      "bytes": 0
    },
    "out/bytesInOutput should be at least 99 (1).js": {
      "imports": [
        {
          "path": "out/111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111-55DNWN2R.file",
          "kind": "file-loader"
        }
      ],
      "exports": [],
      "entryPoint": "project/bytesInOutput should be at least 99 (1).js",
      "inputs": {
        "project/111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111.file": {
          "bytesInOutput": 135
        },
        "project/bytesInOutput should be at least 99 (1).js": {
          "bytesInOutput": 24
        }
      },
      "bytes": 330
    },
    "out/222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222-55DNWN2R.copy": {
      "imports": [],
      "exports": [],
      "inputs": {
        "project/222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222.copy": {
          "bytesInOutput": 0
        }
      },
      "bytes": 0
    },
    "out/bytesInOutput should be at least 99 (2).js": {
      "imports": [
        {
          "path": "out/222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222-55DNWN2R.copy",
          "kind": "import-statement"
        }
      ],
      "exports": [],
      "entryPoint": "project/bytesInOutput should be at least 99 (2).js",
      "inputs": {
        "project/bytesInOutput should be at least 99 (2).js": {
          "bytesInOutput": 149
        }
      },
      "bytes": 203
    },
    "out/bytesInOutput should be at least 99 (3).js": {
      "imports": [
        {
          "path": "out/333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333-DH3FVEAA.js",
          "kind": "dynamic-import"
        }
      ],
      "exports": [],
      "entryPoint": "project/bytesInOutput should be at least 99 (3).js",
      "inputs": {
        "project/bytesInOutput should be at least 99 (3).js": {
          "bytesInOutput": 143
        }
      },
      "bytes": 197
    },
    "out/333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333-DH3FVEAA.js": {
      "imports": [],
      "exports": [],
      "entryPoint": "project/333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333.js",
      "inputs": {
        "project/333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333.js": {
          "bytesInOutput": 0
        }
      },
      "bytes": 0
    },
    "out/444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444-55DNWN2R.file": {
      "imports": [],
      "exports": [],
      "inputs": {
        "project/444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444.file": {
          "bytesInOutput": 0
        }
      },
      "bytes": 0
    },
    "out/bytesInOutput should be at least 99.css": {
      "imports": [
        {
          "path": "out/444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444-55DNWN2R.file",
          "kind": "url-token"
        }
      ],
      "entryPoint": "project/bytesInOutput should be at least 99.css",
      "inputs": {
        "project/bytesInOutput should be at least 99.css": {
          "bytesInOutput": 144
        }
      },
      "bytes": 198
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
@@ -1,171 +0,0 @@
-{
-  "inputs": {
-    "project/111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111.file": {
-      "bytes": 0,
-      "imports": []
-    },
-    "project/bytesInOutput should be at least 99 (1).js": {
-      "bytes": 150,
-      "imports": [
-        {
-          "path": "project/111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111.file",
-          "kind": "import-statement",
-          "original": "./111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111.file"
-        }
-      ],
-      "format": "esm"
-    },
-    "project/222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222.copy": {
-      "bytes": 0,
-      "imports": []
-    },
-    "project/bytesInOutput should be at least 99 (2).js": {
-      "bytes": 150,
-      "imports": [
-        {
-          "path": "project/222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222.copy",
-          "kind": "import-statement",
-          "original": "./222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222.copy"
-        }
-      ],
-      "format": "esm"
-    },
-    "project/333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333.js": {
-      "bytes": 0,
-      "imports": []
-    },
-    "project/bytesInOutput should be at least 99 (3).js": {
-      "bytes": 141,
-      "imports": [
-        {
-          "path": "project/333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333.js",
-          "kind": "dynamic-import",
-          "original": "./333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333.js"
-        }
-      ]
-    },
-    "project/444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444.file": {
-      "bytes": 0,
-      "imports": []
-    },
-    "project/bytesInOutput should be at least 99.css": {
-      "bytes": 136,
-      "imports": [
-        {
-          "path": "project/444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444.file",
-          "kind": "url-token",
-          "original": "444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444.file"
-        }
-      ]
-    }
-  },
-  "outputs": {
-    "out/111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111-55DNWN2R.file": {
-      "imports": [],
-      "exports": [],
-      "inputs": {
-        "project/111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111.file": {
-          "bytesInOutput": 0
-        }
-      },
-      "bytes": 0
-    },
-    "out/bytesInOutput should be at least 99 (1).js": {
-      "imports": [
-        {
-          "path": "out/111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111-55DNWN2R.file",
-          "kind": "file-loader"
-        }
-      ],
-      "exports": [],
-      "entryPoint": "project/bytesInOutput should be at least 99 (1).js",
-      "inputs": {
-        "project/111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111.file": {
-          "bytesInOutput": 135
-        },
-        "project/bytesInOutput should be at least 99 (1).js": {
-          "bytesInOutput": 24
-        }
-      },
-      "bytes": 330
-    },
-    "out/222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222-55DNWN2R.copy": {
-      "imports": [],
-      "exports": [],
-      "inputs": {
-        "project/222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222.copy": {
-          "bytesInOutput": 0
-        }
-      },
-      "bytes": 0
-    },
-    "out/bytesInOutput should be at least 99 (2).js": {
-      "imports": [
-        {
-          "path": "out/222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222-55DNWN2R.copy",
-          "kind": "import-statement"
-        }
-      ],
-      "exports": [],
-      "entryPoint": "project/bytesInOutput should be at least 99 (2).js",
-      "inputs": {
-        "project/bytesInOutput should be at least 99 (2).js": {
-          "bytesInOutput": 149
-        }
-      },
-      "bytes": 203
-    },
-    "out/bytesInOutput should be at least 99 (3).js": {
-      "imports": [
-        {
-          "path": "out/333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333-DH3FVEAA.js",
-          "kind": "dynamic-import"
-        }
-      ],
-      "exports": [],
-      "entryPoint": "project/bytesInOutput should be at least 99 (3).js",
-      "inputs": {
-        "project/bytesInOutput should be at least 99 (3).js": {
-          "bytesInOutput": 143
-        }
-      },
-      "bytes": 197
-    },
-    "out/333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333-DH3FVEAA.js": {
-      "imports": [],
-      "exports": [],
-      "entryPoint": "project/333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333.js",
-      "inputs": {
-        "project/333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333.js": {
-          "bytesInOutput": 0
-        }
-      },
-      "bytes": 0
-    },
-    "out/444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444-55DNWN2R.file": {
-      "imports": [],
-      "exports": [],
-      "inputs": {
-        "project/444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444.file": {
-          "bytesInOutput": 0
-        }
-      },
-      "bytes": 0
-    },
-    "out/bytesInOutput should be at least 99.css": {
-      "imports": [
-        {
-          "path": "out/444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444444-55DNWN2R.file",
-          "kind": "url-token"
-        }
-      ],
-      "entryPoint": "project/bytesInOutput should be at least 99.css",
-      "inputs": {
-        "project/bytesInOutput should be at least 99.css": {
-          "bytesInOutput": 144
-        }
-      },
-      "bytes": 198
-    }
-  }
-}
\ No newline at end of file

```