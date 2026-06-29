import { authenticatedFetch } from "../../lib/auth.ts";
import type { PageLoad } from "./$types.ts";

export const load: PageLoad = async ({ fetch }) => {
  const settingsRes = await authenticatedFetch("/api/settings", {}, fetch);
  const settings = await settingsRes.json();
  return {
    defaultProvider: settings["llm_default_provider"],
    openrouterModel: settings["llm_openrouter_model"],
    genericEndpoint: settings["llm_generic_endpoint"],
    genericModel: settings["llm_generic_model"],
    allowNewDevices: settings["allow_new_devices"],
    apiserverHost: settings["apiserver_host"],
    tlsCert: settings["tls_cert"],
    mailServer: settings["mail_server"],
    mailUsername: settings["mail_username"],
    mailFrom: settings["mail_from"],
    mailTo: settings["mail_to"],
  };
};
