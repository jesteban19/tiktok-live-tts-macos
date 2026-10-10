import { build } from 'esbuild';
import { spawnSync } from 'node:child_process';
import { mkdirSync,copyFileSync,chmodSync,existsSync } from 'node:fs';
import { resolve } from 'node:path';
const root=resolve(import.meta.dirname,'..'); const resources=resolve(root,'src-tauri/resources');mkdirSync(resources,{recursive:true});
if(process.platform!=='darwin') throw new Error('El motor AVSpeechSynthesizer requiere macOS.');
if(Number(process.versions.node.split('.')[0])<22) throw new Error('Usa Node 22+ para preparar el runtime incluido.');
const result=spawnSync('swiftc',['-O','-target',`${process.arch==='arm64'?'arm64':'x86_64'}-apple-macosx13.0`,'-module-cache-path',resolve(root,'.toolchain/swift-cache'),resolve(root,'native/SpeechHelper.swift'),'-o',resolve(resources,'speech-helper')],{stdio:'inherit'});if(result.status!==0)process.exit(result.status??1);
await build({entryPoints:[resolve(root,'bridge/live.mjs')],bundle:true,platform:'node',target:'node22',format:'cjs',outfile:resolve(resources,'bridge.cjs'),external:['bufferutil','utf-8-validate'],logLevel:'info'});
copyFileSync(process.execPath,resolve(resources,'node'));chmodSync(resolve(resources,'node'),0o755);

const nodeLicense=resolve(process.execPath,'../../LICENSE');if(existsSync(nodeLicense))copyFileSync(nodeLicense,resolve(resources,'NODE-LICENSE.txt'));
