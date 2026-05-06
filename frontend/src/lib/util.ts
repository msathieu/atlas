import { formatRelative } from "date-fns";
import { env } from "$env/dynamic/public";

export const API_HOST = env.PUBLIC_API_HOST ? `https://${env.PUBLIC_API_HOST}` : "";
export const API_PUBKEY = env.PUBLIC_API_PUBKEY;

export function formatTime(timestamp: number): string {
  return formatRelative(timestamp * 1000, Date.now());
}
export function getHostnameForDevice(devices: { id: string; hostname: string }[], id: string): string {
  for (const i of devices) {
    if (i.id == id) {
      return i.hostname;
    }
  }
  return "Invalid Device";
}
export function bufferToBase64(buf: ArrayBuffer): string {
  const array = new Uint8Array(buf);
  return array.toBase64();
}
export function base64ToBuffer(encoded: string): ArrayBuffer {
  return Uint8Array.fromBase64(encoded).buffer;
}
export async function encryptToServer(data: string): Promise<ArrayBuffer> {
  let pubkeyStr = API_PUBKEY;
  if (!pubkeyStr) {
    const infoRes = await fetch(`${API_HOST}/api/info`);
    const info = await infoRes.json();
    pubkeyStr = info.pubkey as string;
  }
  const serverPubKey = await crypto.subtle.importKey("spki", base64ToBuffer(pubkeyStr), { name: "RSA-OAEP", hash: "SHA-512" }, false, ["encrypt"]);
  const encoder = new TextEncoder();
  return await crypto.subtle.encrypt({ name: "RSA-OAEP" }, serverPubKey, encoder.encode(data));
}
export async function signRequest(data: string): Promise<string> {
  const keyStr = localStorage.getItem("privkey");
  if (!keyStr) {
    localStorage.clear();
    location.reload();
    throw null;
  }
  const key = await crypto.subtle.importKey("pkcs8", base64ToBuffer(keyStr), "Ed25519", false, ["sign"]);
  const encoder = new TextEncoder();
  return bufferToBase64(await crypto.subtle.sign("Ed25519", key, encoder.encode(data)));
}
