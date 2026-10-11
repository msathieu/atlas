<script lang="ts">
import { Check } from "@lucide/svelte";
import { updateSetting } from "#lib/settings.ts";

let { name, description, val, type, global, options = [] } = $props();
</script>

<h1 class="text-xl">{description}</h1>
<br>
<form
	onsubmit={async (e) => {
		e.preventDefault();
		await updateSetting(name, val, global === "true");
	}}
>
	{#if type === "textarea"}
		<textarea id={name} placeholder={description} class="textarea w-3xl h-100" bind:value={val}></textarea>
	{:else if type === "select"}
		<select id={name} class="select" bind:value={val}>
			{#each options as option}
				<option value={option}>{option}</option>
			{/each}
		</select>
	{:else}
		<input id={name} type="text" placeholder={description} class="input" bind:value={val}>
	{/if}
	<button type="submit" class="btn"><Check /></button>
</form>
<br>
