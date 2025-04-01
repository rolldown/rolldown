# Reason
1. crates/rolldown/tests/esbuild/default/metafile_no_bundle/bypass.md
2. see https://hyrious.me/esbuild-repl/?version=0.23.0&b=e%00a.js%00import+a+from+%27pkg%27%0Aimport+b+from+%27.%2Ffile%27%0Aconsole.log%28%0A%09a%2C%0A%09b%2C%0A%09require%28%27pkg2%27%29%2C%0A%09require%28%27.%2Ffile2%27%29%2C%0A%09import%28%27.%2Fdynamic%27%29%2C%0A%29%0Aexport+let+exported%0A&b=%00b.js%00export+default+function%28%29+%7B%7D%0A&o=%7B%0A++treeShaking%3A+true%2C%0A%22bundle%22%3A+true%2C%0Aformat%3A+%22esm%22%2C%0A%22external%22%3A+%5B%22*%22%5D%0A%7D
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
import a from "pkg";
import b from "./file";



//#region entry.js
console.log(a, b, __require("pkg2"), __require("./file2"), import("./dynamic"));
let exported;
//#endregion

export { exported };
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,4 +1,5 @@
 import a from "pkg";
 import b from "./file";
-console.log(a, b, require("pkg2"), require("./file2"), import("./dynamic"));
-let exported;
+console.log(a, b, __require("pkg2"), __require("./file2"), import("./dynamic"));
+var exported;
+export {exported};

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