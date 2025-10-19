import assert from "node:assert";
import { defaultProvider } from "./lib";

(async () => {
  const mod = await defaultProvider();

  assert(mod.ddd === 100);
})();
