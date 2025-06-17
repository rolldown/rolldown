import assert from "node:assert";
import { statuses } from "./dist/main.js";

assert.deepEqual(
	statuses,
	{ messages: { 200: "ok" } },
	"JSON module should be imported correctly",
);
