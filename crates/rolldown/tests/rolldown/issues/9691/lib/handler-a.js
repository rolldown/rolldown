// Mirrors @smithy node-http-handler.js: a re-export-shaped dep that IS used by
// the consumer's requested export, so its import (node:https) is loaded.
import { dep_a } from "./dep-a.js";

export class HandlerA {
  constants = dep_a.constants;
}
