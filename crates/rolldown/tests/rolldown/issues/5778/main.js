import a, { b, c, d } from "@meojs/cfgs/package.json" with { type: "json", foo: "bar", tag: "baz" };
import foo from "foo" with { type: "foo"};

console.log(`cfgsPackageJson: `,  a, b, c, d);