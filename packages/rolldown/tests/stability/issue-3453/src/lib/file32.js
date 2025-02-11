import{jsx as _jsx,jsxs as _jsxs}from"react/jsx-runtime";import{addPropertyControls,ControlType,// @ts-ignore Internal function
useLocaleInfo,withCSS}from"framer";import{useId,useState}from"react";import{getBorderStyle,borderControls}from"https://framerusercontent.com/modules/cuKUFdzXlhvw8OVOBeAc/T08RxQJ4qrs7LLc8wx4E/border.js";import{getFocusStyle,focusControls}from"https://framerusercontent.com/modules/9muYaW1MvHoRQJ0P7dkP/V2GVvLqiMxXRSxszkCSa/focus.js";import{getHoverStyle,hoverControls}from"https://framerusercontent.com/modules/YfmtnpWjJrP37sQ18QUZ/9Y2P24U2SBIbf2fPVsOX/hover.js";import{getPaddingStyle,paddingControls}from"https://framerusercontent.com/modules/wjZLfSMaP1TvJDu5PCwr/6SPClu354QJPCp6Xj5C0/padding.js";import{getRadiusStyle,radiusControls}from"https://framerusercontent.com/modules/N6MwtHbWoiZJNn1xpqxu/58OHv7BfCzgeBhiv1TYu/radius.js";const className="framer-locale-picker";function addPixel(value){if(typeof value==="number"){return`${value}px`;}return value;}var IconType;(function(IconType){IconType["Default"]="default";IconType["Custom"]="custom";})(IconType||(IconType={}));function Icon({type,color,image,size}){if(type==="custom"&&image){return /*#__PURE__*/_jsx("img",{...image,width:size,height:size});}return /*#__PURE__*/_jsx("svg",{xmlns:"http://www.w3.org/2000/svg",viewBox:"0 0 256 256",width:size,height:size,fill:color,children:/*#__PURE__*/_jsx("path",{d:"M128,24A104,104,0,1,0,232,128,104.11,104.11,0,0,0,128,24Zm87.63,96H175.8c-1.41-28.46-10.27-55.47-25.12-77A88.2,88.2,0,0,1,215.63,120ZM128,215.89c-18.73-20.27-30.09-49-31.77-79.89h63.54C158.09,166.87,146.73,195.62,128,215.89ZM96.23,120c1.68-30.87,13-59.62,31.77-79.89,18.73,20.27,30.09,49,31.77,79.89Zm9.09-77C90.47,64.53,81.61,91.54,80.2,120H40.37A88.2,88.2,0,0,1,105.32,43ZM40.37,136H80.2c1.41,28.46,10.27,55.47,25.12,77A88.2,88.2,0,0,1,40.37,136Zm110.31,77c14.85-21.56,23.71-48.57,25.12-77h39.83A88.2,88.2,0,0,1,150.68,213Z"})});}var CaretType;(function(CaretType){CaretType["Default"]="default";CaretType["Custom"]="custom";})(CaretType||(CaretType={}));function Caret({type,color,image,size}){if(type==="custom"&&image){return /*#__PURE__*/_jsx("img",{...image,width:size,height:size});}return /*#__PURE__*/_jsx("svg",{xmlns:"http://www.w3.org/2000/svg",viewBox:"0 0 12 12",width:size,height:size,children:/*#__PURE__*/_jsx("path",{d:"M 2 4.5 L 6 8.5 L 10 4.5",fill:"none",stroke:color,strokeWidth:1.5,strokeLinecap:"round",strokeLinejoin:"round"})});}/**
 * @framerSupportedLayoutWidth any-prefer-fixed
 * @framerSupportedLayoutHeight any
 * @framerDisableUnlink
 * @framerIntrinsicWidth 120
 * @framerIntrinsicHeight 34
 */const LocaleSelector=withCSS(({font,fillColor,textColor,icon,caret,options:{title,gap,border,hover,focus},style,...props})=>{const id=useId();const{activeLocale,locales,setLocale}=useLocaleInfo();var _activeLocale_id;const activeLocaleId=(_activeLocale_id=activeLocale===null||activeLocale===void 0?void 0:activeLocale.id)!==null&&_activeLocale_id!==void 0?_activeLocale_id:"default";const[lastActiveLocaleId,setLastActiveLocaleId]=useState(activeLocaleId);// The useLocaleInfo hook updates the activeLocale variable inside
// a startTransition to load the translations with Suspense. To make
// the component feel responsive we update our own state without Suspense.
const[selectedLocaleId,setSelectedLocaleId]=useState(activeLocaleId);const selectedLocale=locales.find(locale=>locale.id===selectedLocaleId);// The active locale was updated. Ensure we update our internal state as well.
if(lastActiveLocaleId!==activeLocaleId){setLastActiveLocaleId(activeLocaleId);if(selectedLocaleId!==activeLocaleId){setSelectedLocaleId(activeLocaleId);}}function handleChange(event){const localeId=event.target.value;setSelectedLocaleId(localeId);const locale=locales.find(locale=>locale.id===localeId);setLocale(locale);}var _selectedLocale_name;return /*#__PURE__*/_jsxs("div",{className:className,style:style,children:[/*#__PURE__*/_jsx("label",{htmlFor:id,children:"Select Language"}),/*#__PURE__*/_jsx("select",{id:id,value:selectedLocaleId,onChange:handleChange,// If a navigation occurs from switching locales
// the browser can attempt to autofill the select to the last value
// when you use browser back navigation. We don't want that.
autoComplete:"off",children:locales.map(locale=>/*#__PURE__*/_jsx("option",{value:locale.id,children:locale.name},locale.id))}),/*#__PURE__*/_jsxs("div",{className:"input",style:{...font,"--framer-background-color":fillColor,"--framer-color":textColor,...getPaddingStyle(props),...getRadiusStyle(props),...getBorderStyle(border),...getHoverStyle(hover),...getFocusStyle(focus),gap},children:[icon&&/*#__PURE__*/_jsx("div",{className:"icon",children:/*#__PURE__*/_jsx(Icon,{...icon})}),title&&/*#__PURE__*/_jsx("div",{className:"title",children:(_selectedLocale_name=selectedLocale===null||selectedLocale===void 0?void 0:selectedLocale.name)!==null&&_selectedLocale_name!==void 0?_selectedLocale_name:"English"}),caret&&/*#__PURE__*/_jsx("div",{className:"caret",children:/*#__PURE__*/_jsx(Caret,{...caret})})]})]});},[`
            .${className} {
                position: relative;
            }
        `,`
            .${className} label {
                position: absolute;
                width: 1px;
                height: 1px;
                margin: -1px;
                overflow: hidden;
                white-space: nowrap;
                clip: rect(0 0 0 0);
                clip-path: inset(50%);
            }
        `,`
            .${className} select {
                appearance: none;
                position: absolute;
                opacity: 0;
                top: 0;
                right: 0;
                bottom: 0;
                left: 0;
                cursor: inherit;
                width: 100%;
            }
        `,`
            .${className} .input {
                display: flex;
                justify-content: center;
                align-items: center;
                height: 100%;
                pointer-events: none;
                overflow: hidden;
                background-color: var(--framer-background-color);
                color: var(--framer-color);
                border-color: var(--framer-border-color);
            }
        `,`
            .${className} select:focus-visible + .input  {
                outline: var(--framer-focus-outline, none);
                outline-offset: var(--framer-focus-outline-offset);
            }
        `,`
            .${className}:hover .input {
                background-color: var(--framer-hover-background-color, var(--framer-background-color));
                color: var(--framer-hover-color, var(--framer-color));
                border-color: var(--framer-hover-border-color, var(--framer-border-color));
            }
        `,`
            .${className} .title {
                flex: 1 1 auto;
                white-space: nowrap;
                text-overflow: ellipsis;
                overflow: hidden;
            }
        `,`
            .${className} .icon, .${className} .caret {
                display: flex;
                align-items: center;
            }
        `]);LocaleSelector.displayName="Locale Selector";addPropertyControls(LocaleSelector,{font:{// @ts-ignore
type:ControlType.Font,controls:"extended",defaultFontType:"sans-serif",defaultValue:{fontSize:14,lineHeight:"1.5em"}},fillColor:{type:ControlType.Color,title:"Fill",optional:true,defaultValue:"#eee"},textColor:{type:ControlType.Color,title:"Text",defaultValue:"#000"},...paddingControls,...radiusControls,icon:{type:ControlType.Object,buttonTitle:"Size, Color",optional:true,controls:{type:{type:ControlType.Enum,title:"Icon",options:Object.values(IconType),optionTitles:["Default","Custom"],displaySegmentedControl:true,defaultValue:"default"},color:{type:ControlType.Color,displaySegmentedControl:true,defaultValue:"#000",hidden:props=>props.type!=="default"},image:{type:ControlType.ResponsiveImage,title:"File",allowedFileTypes:["jpg","png","svg"],hidden:props=>props.type!=="custom"},size:{type:ControlType.Number,displayStepper:true,defaultValue:18}}},caret:{type:ControlType.Object,buttonTitle:"Size, Color",optional:true,controls:{type:{type:ControlType.Enum,title:"Icon",options:Object.values(CaretType),optionTitles:["Default","Custom"],displaySegmentedControl:true,defaultValue:"default"},color:{type:ControlType.Color,displaySegmentedControl:true,defaultValue:"#000",hidden:props=>props.type!=="default"},image:{type:ControlType.ResponsiveImage,title:"File",allowedFileTypes:["jpg","png","svg"],hidden:props=>props.type!=="custom"},size:{type:ControlType.Number,displayStepper:true,defaultValue:12}},defaultValue:{}},options:{type:ControlType.Object,title:"Options",buttonTitle:"Border, Hover",controls:{title:{type:ControlType.Boolean,defaultValue:true},gap:{type:ControlType.Number,displayStepper:true,defaultValue:5},border:{type:ControlType.Object,buttonTitle:"Color, Width",optional:true,controls:borderControls},hover:{type:ControlType.Object,buttonTitle:"Fill, Border",optional:true,controls:hoverControls},focus:{type:ControlType.Object,buttonTitle:"Color, Width",controls:focusControls}}}});export default LocaleSelector;
export const __FramerMetadata__ = {"exports":{"default":{"type":"reactComponent","name":"LocaleSelector","slots":[],"annotations":{"framerSupportedLayoutWidth":"any-prefer-fixed","framerSupportedLayoutHeight":"any","framerDisableUnlink":"* @framerIntrinsicWidth 120","framerContractVersion":"1","framerIntrinsicHeight":"34"}},"__FramerMetadata__":{"type":"variable"}}}
//# sourceMappingURL=./LocaleSelector.map