import { authenticatedFetch } from "$lib/auth.ts";
import type { PageLoad } from "./$types.ts";
import { parse, stringify } from "uuid";

export const prerender = false;

export const load: PageLoad = async ({ fetch, params, url }) => {
  const type = url.searchParams.get("type");
  if (type !== "agent" && type !== "development_environment") {
    throw new Error("Invalid container type");
  }
  const id = stringify(parse(params.id));
  const containers = await authenticatedFetch(`/api/${type}s`, {}, fetch);
  for (const container of await containers.json()) {
    if (container.id === id) {
      if (container.state === "Running") {
        const port = url.searchParams.get("port");
        let query = "";
        if (type == "development_environment" && port) {
          const portNumber = parseInt(port);
          query = `?port=${portNumber}`;
        }
        const urlRes = await authenticatedFetch(`/api/${type}s/${id}/open${query}`, {
          method: "POST",
        }, fetch);
        const data = await urlRes.json();
        if (data.ready) {
          const url = new URL(data.url);
          if (url.protocol !== "https:") {
            throw new Error("Invalid protocol");
          }
          globalThis.location.href = url.href;
        }
      } else if (!container.pending) {
        await authenticatedFetch(`/api/${type}s/${id}/state?state=start`, {
          method: "POST",
        }, fetch);
      }
      return;
    }
  }
  throw new Error(`Container doesn't exist`);
};
