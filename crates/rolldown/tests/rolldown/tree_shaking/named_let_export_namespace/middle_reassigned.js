import * as api from './api_reassigned';

// `res` is reassigned, so `IsNotReassigned` is cleared and the namespace
// rewire must NOT apply — downstream readers of `res.used` could be
// observing the post-reassignment value.
export let res = api;
res = { used: 'overridden' };
