function firstString(...values) {
  return values.find(value => typeof value === 'string' && value.trim().length > 0) ?? '';
}
function author(data) {
  return firstString(data.user?.displayId, data.user?.uniqueId, data.uniqueId, data.user?.nickname, data.nickname) || 'usuario';
}
export function eventTime(data) {
  const value = Number(data.common?.createTime ?? data.createTime);
  if (!Number.isFinite(value) || value <= 0) return null;
  const milliseconds = value < 1e12 ? value * 1000 : value;
  return Number.isSafeInteger(milliseconds) ? milliseconds : null;
}
export function toComment(data) {
  return {
    kind: 'comment',
    id: firstString(data.common?.msgId, data.msgId) || `live-${Date.now()}-${Math.random()}`,
    user: author(data), text: firstString(data.content, data.comment),
    timestamp: eventTime(data) ?? 0, source: 'real',
  };
}
export function toViewers(data) {
  // proto v3: total is current viewers; totalUser is cumulative visitors.
  const raw = data.total ?? data.viewerCount;
  if (raw === undefined || raw === null || raw === '') return null;
  const count = Number(raw);
  return Number.isSafeInteger(count) && count >= 0 ? {kind: 'viewers', count} : null;
}
export function toGift(data) {
  const gift = data.gift ?? data.giftDetails ?? {};
  const streak = Number(gift.type ?? gift.giftType ?? data.giftType) === 1;
  if (streak && !(data.repeatEnd === true || Number(data.repeatEnd) === 1)) return null;
  const count = Number(data.repeatCount ?? 1);
  if (!Number.isSafeInteger(count) || count < 1 || count > 1_000_000) return null;
  const user = author(data);
  const giftId = String(data.giftId ?? gift.id ?? 'unknown');
  const group = firstString(...[data.groupId, data.common?.msgId, data.msgId].filter(value => value !== '0'));
  if (!group) return null; // Cannot safely count an unidentified replay.
  return {
    kind: 'gift', id: `gift:${user}:${giftId}:${group}`, user,
    name: firstString(gift.name, gift.giftName, data.giftName, data.extendedGiftInfo?.name) || `Regalo ${giftId}`,
    count, timestamp: eventTime(data) ?? 0, source: 'real',
  };
}
// Preserve the envelope's history flag: the library's high-level CHAT events lose it.
export function liveMessages(result) {
  return {...result, messages: result.messages.filter(message => !message.isHistory)};
}
export class SessionGate {
  constructor() { this.since = null; this.seen = new Set(); }
  connected(time = Date.now()) { this.since = time; }
  disconnected() { this.since = null; }
  accept(event) {
    if (!event || !Number.isSafeInteger(event.timestamp) || event.timestamp <= 0 || this.since === null || event.timestamp < this.since || this.seen.has(event.id)) return false;
    this.seen.add(event.id);
    if (this.seen.size > 5000) this.seen.delete(this.seen.values().next().value);
    return true;
  }
}
export function validUsername(value) { return /^[a-zA-Z0-9_.]{2,32}$/.test(value); }
