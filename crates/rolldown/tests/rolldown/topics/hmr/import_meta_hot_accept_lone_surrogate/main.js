import './dep.js';

const cb = () => {};

// Reported as unsupported, not read as a self-accept or as a non-string dep.
import.meta.hot.accept('./dep\uD800.js', cb);
import.meta.hot.accept(['./dep.js', './dep\uD800.js'], cb);
