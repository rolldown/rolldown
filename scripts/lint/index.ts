import { eslintCompatPlugin, type CreateOnceRule, type VisitorWithHooks } from '@oxlint/plugins';

const banExpectAssertionsRule: CreateOnceRule = {
  createOnce(context) {
    return {
      CallExpression(node) {
        if (
          node.callee.type === 'MemberExpression' &&
          node.callee.object.type === 'Identifier' &&
          node.callee.object.name === 'expect' &&
          node.callee.property.type === 'Identifier' &&
          node.callee.property.name === 'assertions'
        ) {
          context.report({
            message:
              'Fixture tests run concurrently and `expect.assertions` does not work with global expect. Use `vi.fn()` instead.',
            node,
          });
        }
      },
    } as VisitorWithHooks;
  },
};

export default eslintCompatPlugin({
  meta: {
    name: 'rolldown-custom',
  },
  rules: {
    'ban-expect-assertions': banExpectAssertionsRule,
  },
});
