// From: @framerjs/fresco/src/components/utils/useCallbackOnMouseMove.ts
import{useRef,useCallback}from"react";import{Browser}from"https://framerusercontent.com/modules/PJVBcBLmDteTEAZh3J9Z/keXJyjyE9VnzUcDMayjg/browser.js";/**
 * Webkit fires mousemove events if the pointer's coordination changes relative
 * to its container (e.g. if the container scrolls), or when a modifier key is
 * pressed, mousemove would fire even if the cursor did not actually move.
 * This helper compares the cursor position between mouse events, and fire the
 * callback only when its position changes.
 */ export const useCallbackOnMouseMove=(callback,mousePositionRef)=>{const prevPositionRef=useRef(null);return useCallback(event=>{if(!Browser.isSafari())return callback(event);const ref=mousePositionRef?mousePositionRef:prevPositionRef;const{clientX,clientY}=event;const prevCursorPosition=ref.current;ref.current={x:clientX,y:clientY};// Ignore mouse moves unless we have a position. Else it might be an
// element that appears behind the mouse without the mouse moving.
if(!prevCursorPosition){return;}if(prevCursorPosition.x!==clientX||prevCursorPosition.y!==clientY){return callback(event);}},[mousePositionRef,callback]);};
export const __FramerMetadata__ = {"exports":{"useCallbackOnMouseMove":{"type":"variable","annotations":{"framerContractVersion":"1"}},"Point":{"type":"tsType","annotations":{"framerContractVersion":"1"}},"__FramerMetadata__":{"type":"variable"}}}
//# sourceMappingURL=./useCallbackOnMouseMove.map