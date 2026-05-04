import { redirect } from "@sveltejs/kit";
import type { LayoutLoad } from "./$types.ts";
import { connectWS } from "$lib/ws.svelte.ts";
import { API_HOST } from "$lib/util.ts";

export const prerender = true;
export const ssr = false;

export const load: LayoutLoad = async ({ fetch, url }) => {
  if (!localStorage.getItem("jwt") && url.pathname !== "/login") {
    throw redirect(300, "/login");
  }
  const infoRes = await fetch(`${API_HOST}/api/info`);
  if (!infoRes.ok) {
    return { connected: false, info: { hostname: "Disconnected" } };
  }
  const info = await infoRes.json();
  if (info.magic !== "13609285118649036194") {
    return { connected: false, info: { hostname: "Disconnected" } };
  }
  if (info.api_version !== 1) {
    location.reload();
  }
  await connectWS(fetch);
  return { connected: true, info };
};
