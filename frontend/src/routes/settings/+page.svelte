<script lang="ts">
import { authenticatedFetch } from "#lib/auth.ts";
import { updateSetting } from "#lib/settings.ts";
import { API_HOST } from "#lib/util.ts";
import { Check } from "@lucide/svelte";
import Setting from "./Setting.svelte";
import { user } from "#lib/state.svelte.ts";

let { data } = $props();
let defaultProvider = $derived(data.defaultProvider);
let openrouterModel = $derived(data.openrouterModel);
let openrouterKey = $state("");
let genericEndpoint = $derived(data.genericEndpoint);
let genericModel = $derived(data.genericModel);
let genericKey = $state("");
let allowNewDevices = $derived(data.allowNewDevices === "true");
let mailUsername = $derived(data.mailUsername);
let mailPassword = $state("");
let mailFrom = $derived(data.mailFrom);
let apkHash = $derived(data.apkHash);
let developmentEnvironmentSettings = $derived(data.developmentEnvironmentSettings);
let developmentEnvironmentExtensions = $derived(data.developmentEnvironmentExtensions);
let developmentEnvironmentGitconfig = $derived(data.developmentEnvironmentGitconfig);
let developmentEnvironmentPaths = $derived(data.developmentEnvironmentPaths);
let downloadLink: any = $state();
let actuallyDownload = false;

async function requestDownloadLink() {
  const res = await authenticatedFetch("/api/download_auth", { method: "POST" });
  const downloadAuth = await res.json();
  downloadLink.href = `${API_HOST}/api/atlas-agent?auth=${downloadAuth.jwt}&arch=x86_64&os=linux`;
  actuallyDownload = true;
  downloadLink.click();
}
async function restart() {
  await authenticatedFetch("/api/restart", { method: "POST" });
}
</script>

<h1 class="text-2xl">User Settings</h1>
<br />

<Setting name="development_environment_settings" description="Development Environment Settings" val={developmentEnvironmentSettings} global="false" type="textarea" />
<Setting name="development_environment_extensions" description="Development Environment Extensions" val={developmentEnvironmentExtensions} global="false" type="text" />
<Setting name="development_environment_gitconfig" description="Development Environment Git Configuration" val={developmentEnvironmentGitconfig} global="false" type="textarea" />
<Setting name="development_environment_paths" description="Development Environment Persisted Paths" val={developmentEnvironmentPaths} global="false" type="text" />

{#if user.admin}
  <h1 class="text-2xl">System Settings</h1>
  <br />

  <Setting name="llm_default_provider" description="Default LLM Provider" val={defaultProvider} global="true" type="text" />
  <Setting name="llm_openrouter_model" description="Openrouter Model" val={openrouterModel} global="true" type="text" />
  <Setting name="llm_openrouter_key" description="Openrouter API Key" val={openrouterKey} global="true" type="text" />
  <Setting name="llm_generic_endpoint" description="Generic LLM Endpoint" val={genericEndpoint} global="true" type="text" />
  <Setting name="llm_generic_model" description="Generic LLM Model" val={genericModel} global="true" type="text" />
  <Setting name="llm_generic_key" description="Generic LLM API Key" val={genericKey} global="true" type="text" />

  <h1 class="text-xl">Allow New Devices</h1>
  <br />
  <form
  onsubmit={async (e) => {
      e.preventDefault();
      await updateSetting("allow_new_devices", String(allowNewDevices), true);
    }}
>
  <input id="allowNewDevices" type="checkbox" class="checkbox" bind:checked={allowNewDevices} />
  <button class="btn"><Check /></button>
</form>
  <br />

  <Setting name="mail_username" description="Mail Username" val={mailUsername} global="true" type="text" />
  <Setting name="mail_password" description="Mail Password" val={mailPassword} global="true" type="text" />
  <Setting name="mail_from" description="Mail From" val={mailFrom} global="true" type="text" />
  <Setting name="apk_hash" description="APK Hash" val={apkHash} global="true" type="text" />

  <h1 class="text-xl">API Certificate</h1>
  <br />
  <textarea id="tlsCert" disabled class="textarea w-3xl h-100 resize-none">{data.tlsCert}</textarea>
  <br />

  <a
  class="btn"
  href="/api/atlas-agent"
  bind:this={downloadLink}
  onclick={async (e) => {
      if (!actuallyDownload) {
        e.preventDefault();
        await requestDownloadLink();
      } else {
        actuallyDownload = false;
      }
    }}
  download
>Download Agent
  </a>
  <br />

  <h1 class="text-xl">Agent Installation Script</h1>
  <br />
  <textarea id="installationScript" disabled class="textarea w-3xl h-100 resize-none">{data.installationScript}</textarea>
  <br />

  <button
  class="btn"
  onclick={async () => {
      await restart();
    }}
>
    Restart System
  </button>
{/if}
