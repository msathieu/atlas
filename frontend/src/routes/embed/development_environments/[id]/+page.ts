import { authenticatedFetch } from "$lib/auth.ts";
import type { PageLoad } from "./$types.ts";

export const prerender = false;

export const load: PageLoad = async ({ fetch, params }) => {
  const development_environments = await authenticatedFetch(`/api/development_environments`, {}, fetch);
  for (const environment of await development_environments.json()) {
    if (environment.id === params.id) {
      if (environment.state === "Running") {
        const urlRes = await authenticatedFetch(`/api/development_environments/${params.id}/open`, {
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
      } else if (!environment.pending) {
        await authenticatedFetch(`/api/development_environments/${params.id}/start`, {
          method: "POST",
        }, fetch);
      }
      return;
    }
  }
  throw new Error(`Development environment doesn't exist`);
};
