import { authenticatedFetch } from "$lib/auth.ts";
import type { PageLoad } from "./$types.ts";

export const prerender = false;

export const load: PageLoad = async ({ fetch, params }) => {
  const request = await authenticatedFetch(`/api/development_environments/ssh_forward/${params.id}`, {}, fetch);
  const data = await request.json();
  return { development_environment_id: data.development_environment_id, name: data.name };
};
