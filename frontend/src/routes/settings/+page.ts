import { authenticatedFetch } from "../../lib/auth.ts";
import type { PageLoad } from "./$types.ts";

export const load: PageLoad = async ({ fetch }) => {
  const defaultProvider = await authenticatedFetch("/api/settings/llm_default_provider", {}, fetch);
  const openrouterModel = await authenticatedFetch("/api/settings/llm_openrouter_model", {}, fetch);
  const genericEndpoint = await authenticatedFetch("/api/settings/llm_generic_endpoint", {}, fetch);
  const genericModel = await authenticatedFetch("/api/settings/llm_generic_model", {}, fetch);
  const allowNewDevices = await authenticatedFetch("/api/settings/allow_new_devices", {}, fetch);
  const apiserverHost = await authenticatedFetch("/api/settings/apiserver_host", {}, fetch);
  const tlsCert = await authenticatedFetch("/api/settings/tls_cert", {}, fetch);
  const mailServer = await authenticatedFetch("/api/settings/mail_server", {}, fetch);
  const mailUsername = await authenticatedFetch("/api/settings/mail_username", {}, fetch);
  const mailFrom = await authenticatedFetch("/api/settings/mail_from", {}, fetch);
  const mailTo = await authenticatedFetch("/api/settings/mail_to", {}, fetch);

  return {
    defaultProvider: await defaultProvider.json(),
    openrouterModel: await openrouterModel.json(),
    genericEndpoint: await genericEndpoint.json(),
    genericModel: await genericModel.json(),
    allowNewDevices: await allowNewDevices.json(),
    apiserverHost: await apiserverHost.json(),
    tlsCert: await tlsCert.json(),
    mailServer: await mailServer.json(),
    mailUsername: await mailUsername.json(),
    mailFrom: await mailFrom.json(),
    mailTo: await mailTo.json(),
  };
};
