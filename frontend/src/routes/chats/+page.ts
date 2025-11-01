import { authenticatedFetch } from "../../lib/auth.ts";
import type { PageLoad } from "./$types.ts";

export const load: PageLoad = async ({ fetch }) => {
  const chats = await authenticatedFetch("/api/chats", {}, fetch);
  return { chats: await chats.json() };
};
