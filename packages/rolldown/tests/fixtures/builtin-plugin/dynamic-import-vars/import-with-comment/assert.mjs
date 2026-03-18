// @ts-nocheck
import assert from 'node:assert';
import { withCommentBeforeParen, withCommentAfterParen } from './dist/main';

withCommentBeforeParen('module-a').then((m) => {
  assert.strictEqual(m.default, 'a');
});

withCommentAfterParen('module-a').then((m) => {
  assert.strictEqual(m.default, 'a');
});
