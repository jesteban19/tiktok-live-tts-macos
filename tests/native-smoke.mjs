import {spawn} from 'node:child_process';import {createInterface} from 'node:readline';import assert from 'node:assert/strict';
const child=spawn('src-tauri/resources/speech-helper',[],{stdio:['pipe','pipe','pipe']});
const events=[];const output=createInterface({input:child.stdout});let voices=[];
const timeout=setTimeout(()=>{console.error('Timeout del motor nativo',events.map(e=>({kind:e.kind,id:e.id})));child.kill();process.exitCode=1;},20000);
child.stderr.on('data',data=>process.stderr.write(data));
output.on('line',line=>{const event=JSON.parse(line);events.push(event);if(event.kind!=='voices')console.log(Date.now(),event);if(event.kind==='voices'){voices=event.voices;assert.ok(voices.length>0);child.stdin.write(JSON.stringify({command:'speak',id:'smoke-1',text:'Prueba de voz local.',volume:0,rate:0.6})+'\n');}if(event.kind==='finished'){assert.equal(event.id,'smoke-1');assert.ok(events.some(e=>e.kind==='started'&&e.id==='smoke-1'));clearTimeout(timeout);console.log(`Voces: ${voices.length}. Eventos started/finished reales: OK (volumen 0).`);child.stdin.end();}});
child.on('exit',code=>{clearTimeout(timeout);if(code!==0||!events.some(e=>e.kind==='finished')){process.exitCode=1;console.error('Motor terminó sin completar lectura',code);}});
