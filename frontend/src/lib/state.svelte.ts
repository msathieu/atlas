import { invalidateAll } from "$app/navigation";
import { authenticatedFetch } from "./auth.ts";
import { API_HOST } from "./util.ts";

export const wsState: { ws: WebSocket | null; connected: boolean } = $state({ ws: null, connected: false });
export const user = $state({ username: "unknown", admin: false });

export async function connectWS(fn?: (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>) {
  if (!wsState.ws && localStorage.getItem("jwt")) {
    const res = await authenticatedFetch("/api/stream_auth", { method: "POST" }, fn);
    const streamAuth = await res.json();
    wsState.ws = new WebSocket(`${API_HOST}/api/stream?auth=${streamAuth["jwt"]}`);
    wsState.ws.addEventListener("open", (_) => {
      wsState.connected = true;
    });
    wsState.ws.addEventListener("close", (_) => {
      wsState.connected = false;
      wsState.ws = null;
    });
    wsState.ws.addEventListener("message", async (msg) => {
      if (msg.data === "POLL") {
        await invalidateAll();
      }
      wsState.ws?.send("");
    });
  }
}
