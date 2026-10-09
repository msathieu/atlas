import { authenticatedFetch } from "../../lib/auth.ts";
import type { PageLoad } from "./$types.ts";

export const load: PageLoad = async ({ fetch }) => {
	const devices = await authenticatedFetch("/api/devices", {}, fetch);
	return { devices: await devices.json() };
};
