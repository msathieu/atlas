import { formatRelative } from "date-fns";
import { env } from "$env/dynamic/public";

export const API_HOST = env.PUBLIC_API_HOST ? `https://${env.PUBLIC_API_HOST}` : "";

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
