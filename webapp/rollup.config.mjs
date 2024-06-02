import typescript from '@rollup/plugin-typescript';
import typescriptEngine from 'typescript';
import pkg from './package.json' assert {type: 'json'};
import terser from '@rollup/plugin-terser';
import filesize from 'rollup-plugin-filesize';
import {nodeResolve} from '@rollup/plugin-node-resolve';
import css from "rollup-plugin-import-css";

const config = {
    input: './src/main.ts',
    output: [
        {
            file: pkg.main,
            format: 'iife'
        },
    ],
    plugins: [
        typescript({
            tsconfig: './tsconfig.json',
            exclude: '**/*.test.*',
            typescript: typescriptEngine,
            declaration: false,
        }),
        css(),
        nodeResolve(),
        terser(),
        filesize()
    ],
    watch: {
        clearScreen: false
    }
};


export default config;
