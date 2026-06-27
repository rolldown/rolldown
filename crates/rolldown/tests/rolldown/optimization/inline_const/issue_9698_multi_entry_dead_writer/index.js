import { custom_render } from "./flag.js";
import { mount as mount_client } from "./main-client.js";
export function mount(...args){ if(custom_render){ return mount_client(...args); } throw new Error("server"); }
