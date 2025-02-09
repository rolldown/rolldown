import{jsx as _jsx,jsxs as _jsxs}from"react/jsx-runtime";import*as React from"react";import{withCSS}from"framer";const NotFoundPage=()=>{React.useEffect(()=>{let robotsTag=document.querySelector('meta[name="robots"]');if(robotsTag){robotsTag.setAttribute("content","noindex");}else{robotsTag=document.createElement("meta");robotsTag.setAttribute("name","robots");robotsTag.setAttribute("content","noindex");document.head.appendChild(robotsTag);}},[]);return /*#__PURE__*/_jsx("div",{className:"__framer-not-found-page",style:{display:"flex",height:"100vh",alignItems:"center",justifyContent:"center",backgroundColor:"var(--color-background)",fontSize:15},children:/*#__PURE__*/_jsxs("main",{style:{display:"flex",width:"100%",maxWidth:555,flexDirection:"column",alignItems:"center",justifyContent:"center",padding:"0 20px",fontFamily:'"GT Walsheim", sans-serif',fontWeight:400},children:[/*#__PURE__*/_jsx("svg",{xmlns:"http://www.w3.org/2000/svg",width:"30",height:"30",style:{marginBottom:30,color:"var(--color-title)"},children:/*#__PURE__*/_jsx("path",{d:"M 6 2 L 24 2 L 24 11 L 15 11 Z M 6 11 L 15 11 L 24 20 L 15 20 L 15 29 L 6 20 Z",fill:"currentColor"})}),/*#__PURE__*/_jsx("h1",{style:{margin:"0 0 15px 0",color:"var(--color-title)",fontSize:24,fontWeight:700,letterSpacing:-.4,lineHeight:1.3,textAlign:"center"},children:"Page Not Found"}),/*#__PURE__*/_jsxs("div",{style:{marginBottom:30,color:"var(--color-description)",lineHeight:1.5,textAlign:"center"},children:["The page you are looking for does not exist.",/*#__PURE__*/_jsx("br",{}),"Sign up for Framer to publish your own website."]}),/*#__PURE__*/_jsx("a",{href:"https://login.framer.com/sign-up/?ref=site-404&redirect=https%3A%2F%2Fframer.com%2F",role:"button",style:{padding:"12px 24px",backgroundColor:"#09f",borderRadius:10,color:"#fff",fontWeight:500,lineHeight:1.2,textDecoration:"none"},children:"Sign Up for Free"})]})});};const css=[`@font-face {
        font-display: swap;
        font-family: GT Walsheim;
        font-weight: 400;
        src: url(https://www.framer.com/fonts/GT-Walsheim/GT-Walsheim-Regular-subset.woff2) format("woff2"), url(https://www.framer.com/fonts/GT-Walsheim/GT-Walsheim-Regular-subset.woff) format("woff")
    }`,`@font-face {
        font-display: swap;
        font-family: GT Walsheim;
        font-weight: 500;
        src: url(https://www.framer.com/fonts/GT-Walsheim/GT-Walsheim-Medium-subset.woff2) format("woff2"), url(https://www.framer.com/fonts/GT-Walsheim/GT-Walsheim-Medium-subset.woff) format("woff")
    }`,`@font-face {
        font-display: swap;
        font-family: GT Walsheim;
        font-weight: 700;
        src: url(https://www.framer.com/fonts/GT-Walsheim/GT-Walsheim-Bold-subset.woff2) format("woff2"), url(https://www.framer.com/fonts/GT-Walsheim/GT-Walsheim-Bold-subset.woff) format("woff")
    }`,`.__framer-not-found-page {
        --color-background: #ffffff;
        --color-title: #333333;
        --color-description: #777777;
    }`,`@media (prefers-color-scheme: dark) {
        .__framer-not-found-page {
            --color-background: #1b1b1b;
            --color-title: #ffffff;
            --color-description: #cccccc;
        }
    }`];const component=withCSS(NotFoundPage,css);export default component;
export const __FramerMetadata__ = {"exports":{"default":{"type":"reactComponent","name":"component","slots":[],"annotations":{"framerContractVersion":"1"}},"__FramerMetadata__":{"type":"variable"}}}
//# sourceMappingURL=./NotFoundPage.map