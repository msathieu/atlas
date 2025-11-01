<script lang="ts">
  import { goto } from "$app/navigation";
  import { authenticatedFetch } from "$lib/auth";
  import { formatTime } from "$lib/util.ts";
  import { Plus, Trash } from "@lucide/svelte";
  let { data } = $props();

  async function createChat() {
    await authenticatedFetch(`/api/chats`, {
      method: "POST",
    });
  }
  async function deleteChat(id: string) {
    await authenticatedFetch(`/api/chats/${id}`, {
      method: "DELETE",
    });
  }
</script>

<table class="border border-base-content/10 table">
  <thead>
    <tr>
      <th>Summary</th>
      <th>Initial Message</th>
      <th>Last Message</th>
      <th>Delete</th>
    </tr>
  </thead>
  <tbody>
    {#each data.chats as chat}
      <tr
        class="cursor-pointer hover:bg-base-200"
        onclick={async () => {
          await goto(`/chats/${chat.id}`);
        }}
      >
        <td>{chat.summary}</td>
        <td>{formatTime(chat.start_timestamp)}</td>
        <td>{formatTime(chat.end_timestamp)}</td>
        <td>
          <button
            class="btn"
            onclick={async (e) => {
              e.stopPropagation();
              await deleteChat(chat.id);
            }}
          >
            <Trash />
          </button>
        </td>
      </tr>
    {/each}
  </tbody>
</table>
<div class="fab">
  <button
    class="btn btn-circle btn-xl btn-accent"
    onclick={async () => {
      await createChat();
    }}
  >
    <Plus />
  </button>
</div>
