import { authenticatedFetch } from "../../lib/auth.ts";
import type { PageLoad } from "./$types.ts";

export const load: PageLoad = async ({ fetch }) => {
  const userSettingsRes = await authenticatedFetch("/api/user/settings", {}, fetch);
  const userSettings = await userSettingsRes.json();
  const userInfoRes = await authenticatedFetch("/api/user/info");
  const userInfo = await userInfoRes.json();
  if (userInfo.admin) {
    const settingsRes = await authenticatedFetch("/api/settings", {}, fetch);
    const settings = await settingsRes.json();
    const installationScriptRes = await authenticatedFetch("/api/agent_script?arch=x86_64&os=linux", {}, fetch);
    const installationScript = await installationScriptRes.json();
    return {
      defaultProvider: settings["llm_default_provider"],
      openrouterModel: settings["llm_openrouter_model"],
      genericEndpoint: settings["llm_generic_endpoint"],
      genericModel: settings["llm_generic_model"],
      allowNewDevices: settings["allow_new_devices"],
      tlsCert: settings["tls_cert"],
      mailServer: settings["mail_server"],
      mailUsername: settings["mail_username"],
      mailFrom: settings["mail_from"],
      installationScript: installationScript["script"],
      apkHash: settings["apk_hash"],
      developmentEnvironmentSettings: userSettings["development_environment_settings"],
      developmentEnvironmentExtensions: userSettings["development_environment_extensions"],
      developmentEnvironmentGitconfig: userSettings["development_environment_gitconfig"],
      developmentEnvironmentPaths: userSettings["development_environment_paths"],
    };
  } else {
    return {
      developmentEnvironmentSettings: userSettings["development_environment_settings"],
      developmentEnvironmentExtensions: userSettings["development_environment_extensions"],
      developmentEnvironmentGitconfig: userSettings["development_environment_gitconfig"],
      developmentEnvironmentPaths: userSettings["development_environment_paths"],
    };
  }
};
