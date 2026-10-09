import { parse, stringify } from "uuid";
import { authenticatedFetch } from "#lib/auth.ts";
import type { PageLoad } from "./$types.ts";

export const prerender = false;

export const load: PageLoad = async ({ fetch, params }) => {
	const id = stringify(parse(params.id));
	const chat = await authenticatedFetch(`/api/chats/${id}`, {}, fetch);
	return { chat: await chat.json(), id };
};
