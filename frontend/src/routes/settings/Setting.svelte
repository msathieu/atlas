<script lang="ts">
import { Check } from "@lucide/svelte";
import { updateSetting } from "$lib/settings.ts";

let { name, description, val, type } = $props();
</script>

<h1 class="text-xl">{description}</h1>
<br />
<form
  onsubmit={async (e) => {
    e.preventDefault();
    await updateSetting(name, val);
  }}
>
  {#if type === "textarea"}
    <textarea id={name} placeholder={description} class="textarea w-3xl h-100" bind:value={val}></textarea>
  {:else}
    <input id={name} type="text" placeholder={description} class="input" bind:value={val} />
  {/if}
  <button class="btn"><Check /></button>
</form>
<br />
