<script lang="ts">
import { authenticatedFetch } from "$lib/auth.ts";
import { updateSetting } from "$lib/settings.ts";
import { API_HOST } from "$lib/util.ts";
import { Check } from "@lucide/svelte";
import Setting from "./Setting.svelte";

let { data } = $props();
let defaultProvider = $derived(data.defaultProvider);
let openrouterModel = $derived(data.openrouterModel);
let openrouterKey = $state("");
let genericEndpoint = $derived(data.genericEndpoint);
let genericModel = $derived(data.genericModel);
let genericKey = $state("");
let allowNewDevices = $derived(data.allowNewDevices === "true");
let apiserverHost = $derived(data.apiserverHost);
let webserverHost = $derived(data.webserverHost);
let mailServer = $derived(data.mailServer);
let mailUsername = $derived(data.mailUsername);
let mailPassword = $state("");
let mailFrom = $derived(data.mailFrom);
let mailTo = $derived(data.mailTo);
let apkHash = $derived(data.apkHash);
let k8sStorageClass = $derived(data.k8sStorageClass);
let developmentEnvironmentImage = $derived(data.developmentEnvironmentImage);
let developmentEnvironmentSettings = $derived(data.developmentEnvironmentSettings);
let developmentEnvironmentExtensions = $derived(data.developmentEnvironmentExtensions);
let developmentEnvironmentGitconfig = $derived(data.developmentEnvironmentGitconfig);
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

<Setting name="llm_default_provider" description="Default LLM Provider" val={defaultProvider} type="text" />
<Setting name="llm_openrouter_model" description="Openrouter Model" val={openrouterModel} type="text" />
<Setting name="llm_openrouter_key" description="Openrouter API Key" val={openrouterKey} type="text" />
<Setting name="llm_generic_endpoint" description="Generic LLM Endpoint" val={genericEndpoint} type="text" />
<Setting name="llm_generic_model" description="Generic LLM Model" val={genericModel} type="text" />
<Setting name="llm_generic_key" description="Generic LLM API Key" val={genericKey} type="text" />

<h1 class="text-xl">Allow New Devices</h1>
<br />
<form
  onsubmit={async (e) => {
    e.preventDefault();
    await updateSetting("allow_new_devices", String(allowNewDevices));
  }}
>
  <input id="allowNewDevices" type="checkbox" class="checkbox" bind:checked={allowNewDevices} />
  <button class="btn"><Check /></button>
</form>
<br />

<Setting name="apiserver_host" description="API Server Host" val={apiserverHost} type="text" />
<Setting name="webserver_host" description="Web Server Host" val={webserverHost} type="text" />
<Setting name="mail_server" description="Mail Server" val={mailServer} type="text" />
<Setting name="mail_username" description="Mail Username" val={mailUsername} type="text" />
<Setting name="mail_password" description="Mail Password" val={mailPassword} type="text" />
<Setting name="mail_from" description="Mail From" val={mailFrom} type="text" />
<Setting name="mail_to" description="Mail To" val={mailTo} type="text" />
<Setting name="apk_hash" description="APK Hash" val={apkHash} type="text" />
<Setting name="k8s_storageclass" description="Kubernetes Storage Class" val={k8sStorageClass} type="text" />
<Setting name="development_environment_image" description="Development Environment Image" val={developmentEnvironmentImage} type="text" />
<Setting name="development_environment_settings" description="Development Environment Settings" val={developmentEnvironmentSettings} type="textarea" />
<Setting name="development_environment_extensions" description="Development Environment Extensions" val={developmentEnvironmentExtensions} type="text" />
<Setting name="development_environment_gitconfig" description="Development Environment Git Configuration" val={developmentEnvironmentGitconfig} type="textarea" />

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
