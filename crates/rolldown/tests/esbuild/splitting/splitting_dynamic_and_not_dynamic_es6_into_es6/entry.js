import {bar as a} from "./foo.js"
import("./foo.js").then(({bar: b}) => console.log(a, b))