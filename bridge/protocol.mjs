function firstString(...values) {
  return values.find(value=>typeof value==='string'&&value.trim().length>0)??'';
}
export function toComment(data) {
  // 2.5 uses raw tiktok-live-proto v3 messages; legacy adapters used comment/uniqueId.
  const text=firstString(data.content,data.comment);
  const user=firstString(data.user?.displayId,data.user?.uniqueId,data.uniqueId,data.user?.nickname,data.nickname)||'usuario';
  const id=firstString(data.common?.msgId,data.msgId)||`live-${Date.now()}-${Math.random()}`;
  return {kind:'comment',id,user,text,timestamp:Date.now(),source:'real'};
}
export function validUsername(value){ return /^[a-zA-Z0-9_.]{2,32}$/.test(value); }
