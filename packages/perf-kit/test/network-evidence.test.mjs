import { test } from 'node:test';
import assert from 'node:assert/strict';
import { recordNetworkEvidence } from '../lib/network-evidence.mjs';

function recording() {
  const handlers = new Map();
  const recorder = recordNetworkEvidence({
    on: (event, handler) => {
      handlers.set(event, handler);
      return () => handlers.delete(event);
    },
  });
  return { recorder, handlers, emit: (event, data) => handlers.get(`Network.${event}`)(data) };
}
const origin = 'http://localhost:3000';
const request = (requestId, file, overrides = {}) => ({
  requestId,
  timestamp: 10,
  request: {
    url: `${origin}/${file}`,
    initialPriority: 'Low',
    headers: { private: 'synthetic' },
    postData: 'synthetic body',
  },
  initiator: {
    type: 'script',
    stack: {
      callFrames: [{ url: 'about:client' }],
      parent: { callFrames: [{ url: `${origin}/entry.js`, lineNumber: 1, columnNumber: 12 }] },
    },
  },
  ...overrides,
});

test('request priority history joins by id and keeps source locations only', () => {
  const { recorder, handlers, emit } = recording();
  emit('requestWillBeSent', request('one', 'chunk.js'));
  emit('requestWillBeSent', request('two', 'other.js'));
  emit('resourceChangedPriority', { requestId: 'unknown', newPriority: 'High', timestamp: 10.05 });
  emit('resourceChangedPriority', { requestId: 'one', newPriority: 'High', timestamp: 10.1 });
  const resources = [{ name: '/chunk.js' }, { name: '/other.js' }, { name: '/missing.js' }];
  recorder.attach(resources, origin);
  assert.deepEqual(resources[0], {
    name: '/chunk.js',
    networkEvidence: 'matched',
    initiator: { type: 'script', url: '/entry.js', lineNumber: 1, columnNumber: 12 },
    requestPriority: {
      initial: 'Low',
      last: 'High',
      changes: [{ priority: 'High', afterStartMs: 100 }],
    },
  });
  assert.equal(resources[1].requestPriority.last, 'Low');
  assert.equal(resources[2].networkEvidence, 'missing');
  assert.doesNotMatch(JSON.stringify(resources), /synthetic/);
  recorder.stop();
  recorder.stop();
  assert.equal(handlers.size, 0);
});

test('repeated URLs and redirect hops cannot borrow another request evidence', () => {
  const { recorder, emit } = recording();
  emit('requestWillBeSent', request('one', 'repeat.js'));
  emit('requestWillBeSent', request('two', 'repeat.js'));
  emit('requestWillBeSent', request('redirect', 'before.js'));
  emit('requestWillBeSent', request('redirect', 'after.js', { redirectResponse: {} }));
  emit('requestWillBeSent', request('image', 'icons.svg'));
  const resources = [
    'repeat.js',
    'repeat.js',
    'before.js',
    'after.js',
    'icons.svg#a',
    'icons.svg#b',
  ].map((file) => ({ name: `/${file}` }));
  recorder.attach(resources, origin);
  assert.ok(
    resources.every((r) => r.networkEvidence === 'ambiguous' && !r.requestPriority && !r.initiator),
  );
  recorder.stop();
});
