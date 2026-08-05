<script lang="ts">
import { authenticatedFetch } from "$lib/auth.ts";
let { data, params } = $props();

setTimeout(() => {
  globalThis.close();
}, 60000);

async function approveRequest(approved: boolean) {
  await authenticatedFetch(`/api/development_environments/ssh_forward/approve/${params.id}`, {
    method: "POST",
    body: JSON.stringify({ approved }),
  });
  globalThis.close();
}
</script>

<div class="flex">
  <div class="m-auto">
    <fieldset class="fieldset text-xl">
      <p>Approve SSH signing request?</p>
      <p>ID: {data.development_environment_id}</p>
      <p>Name: {data.name}</p>
      <button
        class="btn btn-primary"
        onclick={async () => {
          await approveRequest(true);
        }}>Approve</button>
      <button
        class="btn btn-secondary"
        onclick={async () => {
          await approveRequest(false);
        }}>Reject</button>
    </fieldset>
  </div>
</div>
