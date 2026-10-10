import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';
const root=resolve(import.meta.dirname,'..');
const env={...process.env};
if(existsSync(resolve(root,'.toolchain/cargo/bin/cargo'))){env.CARGO_HOME=resolve(root,'.toolchain/cargo');env.RUSTUP_HOME=resolve(root,'.toolchain/rustup');env.PATH=`${env.CARGO_HOME}/bin:${env.PATH}`;}
// Prefer explicitly configured SDK; CLT can be used independently of full Xcode.
const result=spawnSync(resolve(root,'node_modules/.bin/tauri'),process.argv.slice(2),{cwd:root,env,stdio:'inherit'});
process.exit(result.status??1);
