import { authenticatedFetch } from "$lib/auth.ts";
import type { PageLoad } from "./$types.ts";

export const prerender = false;

let storedData: { id: string; development_environment_id: string; name: string } | null = null;

export const load: PageLoad = async ({ fetch, params }) => {
  if (storedData && storedData.id == params.id) {
    return storedData;
  }
  try {
    const request = await authenticatedFetch(`/api/development_environments/ssh_forward/${params.id}`, { method: "POST" }, fetch);
    const data = await request.json();
    storedData = { id: params.id, development_environment_id: data.development_environment_id, name: data.name };
    return storedData;
  } catch (_error) {
    globalThis.close();
  }
};
