<script lang="ts">
import { authenticatedFetch } from "$lib/auth.ts";
import { user } from "$lib/state.svelte.ts";
let { data } = $props();

let name = $state("");
let nestedContainers = $state(false);

async function createEnvironment() {
  const response = await authenticatedFetch(`/api/development_environments`, {
    method: "POST",
    body: JSON.stringify({ name: name, repo: data.repo, branch: "", nested_containers: nestedContainers }),
  });
  const json = await response.json();
  globalThis.open(`/embed/development_environments/${json.id}`, "_blank", "noopener");
  globalThis.close();
}
</script>

<div class="flex">
  <div class="m-auto">
    <fieldset class="fieldset text-xl">
      <p>Create Development Environment?</p>
      <p>Repo: {data.repo}</p>
      <input id="environment_name" type="text" placeholder="Name" class="input" required bind:value={name} />
      {#if user.admin}
        <p>
          Nested Containers:
          <input id="nested_containers" type="checkbox" class="checkbox" bind:checked={nestedContainers} />
        </p>
      {/if}
      <button
        class="btn btn-primary"
        onclick={async () => {
          await createEnvironment();
        }}>Create</button
      >
      <button
        class="btn btn-secondary"
        onclick={() => {
          globalThis.close();
        }}>Cancel</button
      >
    </fieldset>
  </div>
</div>
