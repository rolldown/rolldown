// CJS module that depends on state.js
const { state } = require('./state.js');

function getConfig(key, defaultValue) {
  if (state.config && typeof state.config === 'object') {
    const value = state.config[key];
    if (value !== undefined) return value;
  }
  return defaultValue;
}

module.exports = { getConfig };
