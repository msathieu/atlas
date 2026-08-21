import { authenticatedFetch } from "$lib/auth.ts";
import type { PageLoad } from "./$types.ts";
import { parse, stringify } from "uuid";

export const prerender = false;

export const load: PageLoad = async ({ fetch, params }) => {
  const id = stringify(parse(params.id));
  const agents = await authenticatedFetch(`/api/agents`, {}, fetch);
  for (const agent of await agents.json()) {
    if (agent.id === id) {
      if (agent.state === "Running") {
        const urlRes = await authenticatedFetch(`/api/agents/${id}/open`, {
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
      } else if (!agent.pending) {
        await authenticatedFetch(`/api/agents/${id}/start`, {
          method: "POST",
        }, fetch);
      }
      return;
    }
  }
  throw new Error(`Agent doesn't exist`);
};
