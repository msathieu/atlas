import { formatRelative } from "date-fns";
import { env } from "$env/dynamic/public";
import { verifyResponseSignature } from "./auth.ts";
import Dexie from "dexie";

export const API_HOST = env.PUBLIC_API_HOST ? `https://${env.PUBLIC_API_HOST}` : "";
export const API_ENCRYPTION_PUBKEY = env.PUBLIC_API_ENCRYPTION_PUBKEY;
export const API_SIGNING_PUBKEY = env.PUBLIC_API_SIGNING_PUBKEY;

let encryptionPubkey = API_ENCRYPTION_PUBKEY;
let signingPubkey = API_SIGNING_PUBKEY;

export async function fetchInfo(fn?: (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>): Promise<Response> {
  if (!fn) {
    fn = fetch;
  }
  const infoRes = await fn(`${API_HOST}/api/info`);
  await verifyResponseSignature(infoRes, "info");
  return infoRes;
}
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
  if (!encryptionPubkey) {
    const infoRes = await fetchInfo();
    const info = await infoRes.json();
    encryptionPubkey = info.encryption_pubkey as string;
  }
  const serverPubKey = await crypto.subtle.importKey("spki", base64ToBuffer(encryptionPubkey), { name: "RSA-OAEP", hash: "SHA-512" }, false, ["encrypt"]);
  const encoder = new TextEncoder();
  return await crypto.subtle.encrypt({ name: "RSA-OAEP" }, serverPubKey, encoder.encode(data));
}
export async function signRequest(data: string): Promise<string> {
  const db = getDatabase();
  const record = await db.table("keys").get("jwt_signing");
  const key = record?.key;
  if (!key) {
    localStorage.removeItem("jwt");
    location.reload();
    throw new Error("No private key in storage");
  }
  const encoder = new TextEncoder();
  return bufferToBase64(await crypto.subtle.sign("Ed25519", key, encoder.encode(data)));
}
export async function verifySignature(data: string, signature: string): Promise<boolean> {
  if (!signingPubkey) {
    const infoRes = await fetch(`${API_HOST}/api/info`);
    const info = await infoRes.json();
    signingPubkey = info.signing_pubkey as string;
  }
  const pubkey = await crypto.subtle.importKey("spki", base64ToBuffer(signingPubkey), "Ed25519", false, ["verify"]);
  const encoder = new TextEncoder();
  return await crypto.subtle.verify("Ed25519", pubkey, base64ToBuffer(signature), encoder.encode(data));
}
export function getDatabase(): Dexie {
  const db = new Dexie("atlas");
  db.version(1).stores({
    keys: "name",
  });
  return db;
}
