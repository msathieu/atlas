import { authenticatedFetch } from "$lib/auth.ts";
import type { PageLoad } from "./$types.ts";

export const prerender = false;

export const load: PageLoad = async ({ fetch, params }) => {
  const chat = await authenticatedFetch(`/api/chats/${params.id}`, {}, fetch);
  return { chat: await chat.json(), id: params.id };
};
