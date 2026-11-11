import { defineConfig } from 'vite';
import { nodePolyfills } from 'vite-plugin-node-polyfills';
export default defineConfig({plugins:[nodePolyfills({include:['buffer','process','util'],globals:{Buffer:true,process:true,global:true}})],server:{host:'127.0.0.1',port:8932,strictPort:true}});
