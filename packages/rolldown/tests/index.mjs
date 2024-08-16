// const path = require("path");
import * as path from "path";
import { $, execa } from "execa";
const binPath =

	"/home/victor/Documents/rolldown-rs/rolldown/packages/rolldown/bin/cli.js";
console.log(`binPath: `, binPath);
const stdout = await execa`node ${binPath} --help`;

console.log(`stdout: `, stdout);
