import { refreshAll } from "$app/navigation";
import { page } from "$app/state";
import { authenticatedFetch } from "./auth.ts";
import { API_HOST } from "./util.ts";

export const wsState: { ws: WebSocket | null; connected: boolean } = $state({ ws: null, connected: false });
export const user = $state({ username: "unknown", admin: false });
export const mounted = $state({ mounted: false });

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
      const sshPrefix = "SSH_FORWARD:";
      if (msg.data === "POLL" && mounted.mounted) {
        await refreshAll();
      } else if (msg.data.startsWith(sshPrefix)) {
        if (!page.url.pathname.startsWith("/embed/")) {
          const signingId = msg.data.slice(sshPrefix.length);
          globalThis.open(`/embed/ssh_forward/${signingId}`, "_blank", "popup");
        }
      }
      wsState.ws?.send("");
    });
  }
}
