import { test } from 'node:test';import assert from 'node:assert/strict';import {toComment,validUsername} from './protocol.mjs';
test('evento CHAT real conserva identificador, autor y procedencia',()=>{const c=toComment({common:{msgId:'123'},user:{uniqueId:'ana'},comment:'Hola'});assert.equal(c.id,'123');assert.equal(c.user,'ana');assert.equal(c.source,'real');assert.equal(c.text,'Hola');});
test('rechaza rutas y argumentos como usuario',()=>{assert.equal(validUsername('ana.dev'),true);for(const u of ['../../tmp','--help','','ana dev'])assert.equal(validUsername(u),false);});
test('regresión 2.5: content y displayId del evento proto v3 llegan al TTS',()=>{const c=toComment({common:{msgId:'7591023'},user:{displayId:'carlos.dev',nickname:'Carlos P'},content:'¿Puedes explicar este ejemplo?',comment:undefined});assert.equal(c.text,'¿Puedes explicar este ejemplo?');assert.equal(c.user,'carlos.dev');assert.equal(c.id,'7591023');assert.equal(c.source,'real');assert.ok(c.text.trim().length>0);});
test('ignora objetos de contenido desconocido y no inventa texto',()=>{assert.equal(toComment({content:{svg:'<svg />'},comment:undefined}).text,'');assert.equal(toComment({user:{displayId:'',nickname:'Ana'},content:'Hola'}).user,'Ana');});

// Realistic payloads match the installed proto v3 schema, not outdated README examples.
import {eventTime, toViewers, toGift, liveMessages, SessionGate} from './protocol.mjs';
test('hora original se conserva en segundos y milisegundos; no se inventa para eventos antiguos',()=>{
  assert.equal(toComment({common:{createTime:'1791650000'},content:'Hola'}).timestamp,1791650000000);
  assert.equal(eventTime({common:{createTime:'1791650000123'}}),1791650000123);
  for(const value of [undefined,'0','bad','-1','Infinity']) assert.equal(eventTime({common:{createTime:value}}),null);
});
test('conexión y reconexión rechazan historial, hora desconocida y replay de IDs',()=>{
  const gate=new SessionGate();const comment={id:'1',timestamp:2000};
  assert.equal(gate.accept(comment),false);
  gate.connected(1501);
  assert.equal(gate.accept({id:'old',timestamp:1000}),false);
  assert.equal(gate.accept({id:'unknown',timestamp:0}),false);
  assert.equal(gate.accept({id:'missing'}),false);
  assert.equal(gate.accept(comment),true);
  assert.equal(gate.accept(comment),false);
  gate.disconnected();assert.equal(gate.accept({id:'2',timestamp:3000}),false);
  gate.connected(3500);
  assert.equal(gate.accept({id:'2',timestamp:3000}),false);
  assert.equal(gate.accept({id:'3',timestamp:4000}),true);
  // Reopening the application creates a new gate with a later boundary.
  const reopened=new SessionGate();reopened.connected(5000);assert.equal(reopened.accept(comment),false);
});
test('descarta el flag de historial antes de perder la envoltura proto',()=>{
  const old={isHistory:true,decodedData:{type:'WebcastChatMessage'}};
  const fresh={isHistory:false,decodedData:{type:'WebcastChatMessage'}};
  const input={messages:[old,fresh],cursor:'cursor'};const output=liveMessages(input);
  assert.deepEqual(output.messages,[fresh]);assert.equal(input.messages.length,2);assert.equal(output.cursor,'cursor');
});
test('contador usa espectadores actuales y nunca los visitantes acumulados',()=>{
  assert.deepEqual(toViewers({total:'42',totalUser:'14000'}),{kind:'viewers',count:42});
  assert.deepEqual(toViewers({viewerCount:0}),{kind:'viewers',count:0});
  for(const input of [{totalUser:'100'}, {total:'-1'}, {total:'bad'}, {total:''}]) assert.equal(toViewers(input),null);
});
test('regalos proto v3: ignora incrementos y cuenta solo la racha final, una vez',()=>{
  const data={common:{msgId:'10',createTime:'1791650000'},user:{displayId:'ana'},giftId:'5655',groupId:'streak-1',gift:{type:1,name:'Rosa'},repeatCount:3,repeatEnd:0};
  assert.equal(toGift(data),null);
  const final=toGift({...data,repeatEnd:1});assert.equal(final.name,'Rosa');assert.equal(final.count,3);assert.equal(final.user,'ana');assert.equal(final.source,'real');
  const gate=new SessionGate();gate.connected(1791649999000);assert.equal(gate.accept(final),true);
  assert.equal(gate.accept(toGift({...data,repeatEnd:1,common:{...data.common,msgId:'11'}})),false);
  assert.equal(toGift({...data,gift:{type:2,name:'Corazón'},repeatCount:1}).count,1);
  const single={...data,groupId:'0',gift:{type:2,name:'Corazón'},repeatCount:1};
  assert.notEqual(toGift(single).id,toGift({...single,common:{...data.common,msgId:'different'}}).id);
  for(const repeatCount of [0,-1,NaN,1.5]) assert.equal(toGift({...data,repeatEnd:1,repeatCount}),null);
});
