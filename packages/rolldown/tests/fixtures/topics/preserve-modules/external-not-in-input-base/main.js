// Companion to ../issue-10186: the same rooted id, but resolved as external.
// External modules never become preserved chunks, so this import must not
// influence where the real chunks land.
import '/favicon';

export const ok = true;
