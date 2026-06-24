import { UsedIcon } from './icon-lib.js';
import { createVNode, defineComponent, unref } from './vue.js';

export default /* @__PURE__ */ defineComponent({
  render() {
    return createVNode(unref(UsedIcon));
  },
});
