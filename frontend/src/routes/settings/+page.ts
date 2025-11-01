import { authenticatedFetch } from "../../lib/auth.ts";
import type { PageLoad } from "./$types.ts";

export const load: PageLoad = async ({ fetch }) => {
  const openrouterModel = await authenticatedFetch("/api/settings/openrouter_model", {}, fetch);
  const allowNewDevices = await authenticatedFetch("/api/settings/allow_new_devices", {}, fetch);
  const apiserverHost = await authenticatedFetch("/api/settings/apiserver_host", {}, fetch);
  const tlsCert = await authenticatedFetch("/api/settings/tls_cert", {}, fetch);

  return { openrouterModel: await openrouterModel.json(), allowNewDevices: await allowNewDevices.json(), apiserverHost: await apiserverHost.json(), tlsCert: await tlsCert.json() };
};
