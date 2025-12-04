<script lang="ts">
  import { authenticatedFetch } from "$lib/auth.ts";
  import { API_HOST } from "$lib/util.js";
  import { Check } from "@lucide/svelte";
  import { toast } from "svelte-sonner";

  let { data } = $props();
  let openrouterModel = $derived(data.openrouterModel.value);
  let openrouterKey = $state("");
  let allowNewDevices = $derived(data.allowNewDevices.value === "true");
  let apiServerHost = $derived(data.apiserverHost.value);
  let downloadLink: any = $state();
  let actuallyDownload = false;

  async function updateSetting(name: string, value: string) {
    const response = await authenticatedFetch(`/api/settings/${name}`, {
      method: "PUT",
      body: JSON.stringify({ value }),
    });
    if (response.ok) {
      toast.success("Updated Setting");
    }
  }
  async function requestDownloadLink() {
    const res = await authenticatedFetch("/api/download_auth", { method: "POST" });
    const downloadAuth = await res.json();
    downloadLink.href = `${API_HOST}/api/atlas-agent?auth=${downloadAuth.jwt}`;
    actuallyDownload = true;
    downloadLink.click();
  }
  async function restart() {
    await authenticatedFetch("/api/restart", { method: "POST" });
  }
</script>

<h1 class="text-xl">Openrouter Model</h1>
<br />
<form
  onsubmit={async () => {
    await updateSetting("openrouter_model", openrouterModel);
  }}
>
  <input type="text" placeholder="Openrouter Model" class="input" bind:value={openrouterModel} />
  <button class="btn"><Check /></button>
</form>
<br />

<h1 class="text-xl">Openrouter API Key</h1>
<br />
<form
  onsubmit={async () => {
    await updateSetting("openrouter_key", openrouterKey);
  }}
>
  <input type="text" placeholder="Openrouter API Key" class="input" bind:value={openrouterKey} />
  <button class="btn"><Check /></button>
</form>
<br />

<h1 class="text-xl">Allow New Devices</h1>
<br />
<form
  onsubmit={async () => {
    await updateSetting("allow_new_devices", String(allowNewDevices));
  }}
>
  <input type="checkbox" class="checkbox" bind:checked={allowNewDevices} />
  <button class="btn"><Check /></button>
</form>
<br />

<h1 class="text-xl">API Server Host</h1>
<br />
<form
  onsubmit={async () => {
    await updateSetting("apiserver_host", String(apiServerHost));
  }}
>
  <input type="text" placeholder="API Server Host" class="input" bind:value={apiServerHost} />
  <button class="btn"><Check /></button>
</form>
<br />

<h1 class="text-xl">API Certificate</h1>
<br />
<textarea disabled class="textarea w-3xl h-100 resize-none">{data.tlsCert.value}</textarea>
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

<button
  class="btn"
  onclick={async () => {
    await restart();
  }}
>
  Restart System
</button>
