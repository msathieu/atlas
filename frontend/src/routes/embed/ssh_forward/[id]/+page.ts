import { authenticatedFetch } from "#lib/auth.ts";
import type { PageLoad } from "./$types.ts";
import { parse, stringify } from "uuid";

export const prerender = false;

let storedData: { id: string; development_environment_id: string; name: string } | null = null;

export const load: PageLoad = async ({ fetch, params }) => {
  const id = stringify(parse(params.id));
  if (storedData && storedData.id == id) {
    return storedData;
  }
  try {
    const request = await authenticatedFetch(`/api/development_environments/ssh_forward/${id}`, { method: "POST" }, fetch);
    const data = await request.json();
    storedData = { id, development_environment_id: data.development_environment_id, name: data.name };
    return storedData;
  } catch (_error) {
    globalThis.close();
  }
};
