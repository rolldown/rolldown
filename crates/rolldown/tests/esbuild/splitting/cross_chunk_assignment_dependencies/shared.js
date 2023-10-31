var observer;
var value;
export function setObserver(cb) {
    observer = cb;
}
export function getValue() {
    return value;
}
export function setValue(next) {
    value = next;
    if (observer) observer();
}
sideEffects(getValue);