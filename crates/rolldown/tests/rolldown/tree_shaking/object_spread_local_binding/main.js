// Spreading a local binding should not be treated as having side effects.
// This is the pattern used by libraries like Effect (effect-ts).
const Proto = {
  typeId: '~test/Proto',
  pipe() {
    return this;
  },
  toJSON() {
    return { _tag: this.typeId };
  },
};

// These bindings should be tree-shaken: ProtoUtc/ProtoZoned are unused and spread of a const plain object is safe
const ProtoUtc = { ...Proto, _tag: 'Utc' };
const ProtoZoned = { ...Proto, _tag: 'Zoned' };

// Only this should survive
export const used = Proto.typeId;
