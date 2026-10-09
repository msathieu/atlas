import { authenticatedFetch } from "../../lib/auth.ts";
import type { PageLoad } from "./$types.ts";

export const load: PageLoad = async ({ fetch }) => {
	const commands = await authenticatedFetch("/api/commands", {}, fetch);
	const devices = await authenticatedFetch("/api/devices", {}, fetch);
	return { commands: await commands.json(), devices: await devices.json() };
};
