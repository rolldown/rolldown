import assert from "node:assert";
import { statuses } from "./dist/main.js";

assert.deepEqual(
	statuses.code1,
	{ 200: "ok", foo: "bar" },
	"JSON module code1 should be imported correctly",
);

assert.deepEqual(
	statuses.code2,
	{  200: "ok", foo: "bar" },
	"JSON module code2 should be imported correctly",
);
