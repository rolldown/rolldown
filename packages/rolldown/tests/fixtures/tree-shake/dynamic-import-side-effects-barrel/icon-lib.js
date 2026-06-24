import { createVNode, defineComponent as component, isVNode } from './vue.js';

const slice = Array.prototype.slice;
function createElement(tag, props = null, children = null) {
  if (arguments.length > 3 || isVNode(children)) {
    children = slice.call(arguments, 2);
  }
  return createVNode(tag, props, children);
}

const UsedIcon = component({
    name: 'UsedIcon',
    props: {
      size: { type: String, default: '24px' },
    },
    render() {
      return createElement('svg', { width: this.size });
    },
  }),
  UnusedIcon = component({
    name: 'UnusedIcon',
    props: {
      size: { type: String, default: '24px' },
    },
    render() {
      return createElement('svg', { width: this.size });
    },
  });

export { UsedIcon, UnusedIcon };
