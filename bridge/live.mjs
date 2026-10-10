import { TikTokLiveConnection, ControlEvent, WebcastEvent, UserOfflineError } from 'tiktok-live-connector';
import { toComment,validUsername } from './protocol.mjs';
import { createInterface } from 'node:readline';
// stdout is exclusively JSONL; library diagnostics go to stderr.
console.log=(...args)=>console.error(...args);
const send=(value)=>process.stdout.write(JSON.stringify(value)+'\n');
const username=process.argv[2];let stopped=false,connection,attempt=0,timer;
if(!validUsername(username??'')){send({kind:'status',status:'error',message:'Usuario TikTok inválido.'});process.exit(1);}
async function stop(){stopped=true;clearTimeout(timer);connection?.removeAllListeners();await connection?.disconnect();process.exit(0);}
function retry(message){if(stopped||timer)return; if(attempt>=5){send({kind:'status',status:'error',message:'No se pudo conectar tras 5 intentos. '+message});return;}const delay=Math.min(30000,1000*2**attempt++);send({kind:'status',status:'reconnecting',message:`Reintentando en ${delay/1000}s. ${message}`});timer=setTimeout(()=>{timer=undefined;void connect();},delay);}
async function connect(){
  if(stopped)return; connection?.removeAllListeners();await connection?.disconnect();
  connection=new TikTokLiveConnection(username,{processInitialData:false,fetchRoomInfoOnConnect:true,authenticateWs:false,signApiKey:process.env.EULER_API_KEY||undefined});
  connection.on(ControlEvent.CONNECTED,()=>{attempt=0;send({kind:'status',status:'connected',message:'Conectado a TikTok LIVE (no oficial).'});});
  connection.on(WebcastEvent.CHAT,data=>send(toComment(data)));
  connection.on(WebcastEvent.STREAM_END,()=>{stopped=true;send({kind:'status',status:'ended',message:'La transmisión terminó.'});void connection.disconnect();});
  connection.on(ControlEvent.DISCONNECTED,()=>{if(!stopped)retry('Conexión interrumpida.');});
  connection.on(ControlEvent.ERROR,error=>send({kind:'notice',message:String(error?.message??error?.info??'Error del conector TikTok.')}));
  try{await connection.connect();}catch(error){const message=String(error?.message??error);if(error instanceof UserOfflineError){stopped=true;send({kind:'status',status:'offline',message:'El usuario no está transmitiendo.'});}else retry(message);}
}
createInterface({input:process.stdin}).on('line',line=>{if(line.trim()==='stop')void stop();}).on('close',()=>void stop());
process.on('SIGTERM',()=>void stop());process.on('SIGINT',()=>void stop());void connect();
