<script lang="ts">
import { goto } from "$app/navigation";
import { authenticatedFetch } from "$lib/auth";
import { formatTime } from "$lib/util.ts";
import { Bot, MessageCircle, Plus, Trash } from "@lucide/svelte";
let { data } = $props();

async function createChat(chat_type: string) {
  await authenticatedFetch(`/api/chats`, {
    method: "POST",
    body: JSON.stringify({ chat_type }),
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
      <th>Type</th>
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
        <td>
          {#if chat.chat_type == "interactive"}<MessageCircle />{:else if chat.chat_type == "noninteractive"}<Bot />{/if}
        </td>
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
<div class="fab fab-flower">
  <div tabindex="0" role="button" class="btn btn-circle btn-xl btn-accent">
    <Plus />
  </div>
  <button class="fab-main-action btn btn-circle btn-xl btn-accent">
    <Plus />
  </button>
  <button
    class="btn btn-circle btn-xl btn-accent"
    onclick={async () => {
      await createChat("interactive");
    }}
  >
    <MessageCircle />
  </button>
  <button
    class="btn btn-circle btn-xl btn-accent"
    onclick={async () => {
      await createChat("noninteractive");
    }}
  >
    <Bot />
  </button>
</div>
