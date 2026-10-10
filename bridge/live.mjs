import { TikTokLiveConnection, ControlEvent, WebcastEvent, UserOfflineError } from 'tiktok-live-connector';
import { toComment, toViewers, toGift, liveMessages, SessionGate, validUsername } from './protocol.mjs';
import { createInterface } from 'node:readline';
// stdout is exclusively JSONL; library diagnostics go to stderr.
console.log=(...args)=>console.error(...args);
const send=(value)=>process.stdout.write(JSON.stringify(value)+'\n');
const username=process.argv[2];let stopped=false,connection,attempt=0,timer;
const gate = new SessionGate();
class FreshLiveConnection extends TikTokLiveConnection {
  async processProtoMessageFetchResult(result) {
    return super.processProtoMessageFetchResult(liveMessages(result));
  }
}
if(!validUsername(username??'')){send({kind:'status',status:'error',message:'Usuario TikTok inválido.'});process.exit(1);}
async function stop(){stopped=true;clearTimeout(timer);connection?.removeAllListeners();await connection?.disconnect();process.exit(0);}
function retry(message){if(stopped||timer)return; if(attempt>=5){send({kind:'status',status:'error',message:'No se pudo conectar tras 5 intentos. '+message});return;}const delay=Math.min(30000,1000*2**attempt++);send({kind:'status',status:'reconnecting',message:`Reintentando en ${delay/1000}s. ${message}`});timer=setTimeout(()=>{timer=undefined;void connect();},delay);}
async function connect(){
  if(stopped)return; connection?.removeAllListeners();await connection?.disconnect();
  gate.disconnected();
  connection=new FreshLiveConnection(username,{processInitialData:false,fetchRoomInfoOnConnect:true,authenticateWs:false,signApiKey:process.env.EULER_API_KEY||undefined});
  connection.on(ControlEvent.CONNECTED,()=>{gate.connected();attempt=0;send({kind:'status',status:'connected',message:'Conectado a TikTok LIVE (no oficial). Solo mensajes nuevos.'});});
  connection.on(WebcastEvent.CHAT,data=>{const event=toComment(data);if(gate.accept(event))send(event);});
  connection.on(WebcastEvent.ROOM_USER,data=>{const event=toViewers(data);if(gate.since!==null&&event)send(event);});
  connection.on(WebcastEvent.GIFT,data=>{const event=toGift(data);if(gate.accept(event))send(event);});
  connection.on(WebcastEvent.STREAM_END,()=>{stopped=true;send({kind:'status',status:'ended',message:'La transmisión terminó.'});void connection.disconnect();});
  connection.on(ControlEvent.DISCONNECTED,()=>{gate.disconnected();if(!stopped)retry('Conexión interrumpida.');});
  connection.on(ControlEvent.ERROR,error=>send({kind:'notice',message:String(error?.message??error?.info??'Error del conector TikTok.')}));
  try{await connection.connect();}catch(error){const message=String(error?.message??error);if(error instanceof UserOfflineError){stopped=true;send({kind:'status',status:'offline',message:'El usuario no está transmitiendo.'});}else retry(message);}
}
createInterface({input:process.stdin}).on('line',line=>{if(line.trim()==='stop')void stop();}).on('close',()=>void stop());
process.on('SIGTERM',()=>void stop());process.on('SIGINT',()=>void stop());void connect();
