<script lang="ts">
  import { Check } from "@lucide/svelte";
  import { updateSetting } from "$lib/settings.ts";

  let { name, description, val } = $props();
</script>

<h1 class="text-xl">{description}</h1>
<br />
<form
  onsubmit={async () => {
    await updateSetting(name, val);
  }}
>
  <input type="text" placeholder={description} class="input" bind:value={val} />
  <button class="btn"><Check /></button>
</form>
<br />
