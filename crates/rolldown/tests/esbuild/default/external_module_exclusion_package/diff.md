# Diff
## /out.js
### esbuild
```js
// index.js
import { S3 } from "aws-sdk";
import { DocumentClient } from "aws-sdk/clients/dynamodb";
var s3 = new S3();
var dynamodb = new DocumentClient();
export {
  dynamodb,
  s3
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
-import {S3} from "aws-sdk";
-import {DocumentClient} from "aws-sdk/clients/dynamodb";
-var s3 = new S3();
-var dynamodb = new DocumentClient();
-export {dynamodb, s3};

```