import { redirect } from "@sveltejs/kit";
import type { LayoutLoad } from "./$types.ts";
import { connectWS } from "$lib/ws.svelte.ts";
import { API_ENCRYPTION_PUBKEY, API_SIGNING_PUBKEY, fetchInfo } from "$lib/util.ts";

export const prerender = true;
export const ssr = false;

export const load: LayoutLoad = async ({ fetch, url }) => {
  if (!localStorage.getItem("jwt") && url.pathname !== "/login") {
    throw redirect(300, "/login");
  }
  const infoRes = await fetchInfo(fetch);
  if (!infoRes.ok) {
    return { client_error: false, connected: false, info: { hostname: "Disconnected" } };
  }
  const info = await infoRes.json();
  if (
    info.magic !== "13609285118649036194" || info.api_version !== 1 || (API_ENCRYPTION_PUBKEY && info.encryption_pubkey !== API_ENCRYPTION_PUBKEY) ||
    (API_SIGNING_PUBKEY && info.signing_pubkey !== API_SIGNING_PUBKEY)
  ) {
    return { client_error: true, connected: false, info: { hostname: "Disconnected" } };
  }
  await connectWS(fetch);
  return { client_error: false, connected: true, info };
};
