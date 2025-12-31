<script lang="ts">
  import { authenticatedFetch } from "$lib/auth.ts";
  import { BadgeCheck, BadgeX, Bot, CircleUser, SendHorizontal } from "@lucide/svelte";
  import { marked } from "marked";
  import DOMPurify from "dompurify";
  let { data } = $props();
  let content = $state("");

  async function sendMessage() {
    await authenticatedFetch(`/api/chats/${data.id}`, {
      method: "POST",
      body: JSON.stringify({ content }),
    });
    content = "";
  }
  function disableSend(): boolean {
    if (data.chat.messages.length === 0) {
      return false;
    }
    const last = data.chat.messages.at(-1);
    return last.user_message === last.finished;
  }
  async function manageToolCallConsent(seq: number, callId: string, approved: boolean) {
    let rejection_reason = "";
    if (!approved) {
      rejection_reason = prompt("Reason") || "no reason supplied";
    }
    await authenticatedFetch(`/api/chats/${data.id}/${seq}/${callId}/consent`, {
      method: "POST",
      body: JSON.stringify({ approved, rejection_reason }),
    });
  }
  function isCallPendingConsent(toolCalls: any[]): boolean {
    for (const call of toolCalls) {
      if (call.user_consented === null) {
        return true;
      }
    }
    return false;
  }
</script>

{#each data.chat.messages as message}
  <div class="chat" class:chat-start={!message.user_message} class:chat-end={message.user_message}>
    <div class="chat-image dropdown dropdown-hover">
      <div>
        {#if message.user_message}
          <CircleUser size="48" />
        {:else}
          <Bot size="48" />
        {/if}
      </div>
      {#if !message.user_message}
        <div class="dropdown-content menu bg-base-200 w-50">
          <p>Model: {message.model}</p>
          <p>Input Tokens: {message.input_tokens}</p>
          <p>Output Tokens: {message.output_tokens}</p>
        </div>
      {/if}
    </div>
    <div class="chat-bubble whitespace-pre-wrap">
      {#if message.reasoning}
        <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
        <div class="border collapse collapse-arrow" tabindex="0">
          <div class="collapse-title">Reasoning</div>
          <div class="collapse-content">
            {@html DOMPurify.sanitize(marked(message.reasoning) as string)}
          </div>
        </div>
      {/if}
      {#if message.tool_calls.length !== 0}
        <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
        <div class="border collapse collapse-arrow" class:bg-primary={isCallPendingConsent(message.tool_calls)} tabindex="0">
          <div class="collapse-title">Tool Calls</div>
          <div class="collapse-content">
            {#each message.tool_calls as call}
              <table class="table">
                <tbody>
                  <tr>
                    <td>Tool</td>
                    <td>{call.tool}</td>
                  </tr>
                  <tr>
                    <td>Request</td>
                    <td class="whitespace-pre-wrap">{JSON.stringify(JSON.parse(call.request), null, 2).replaceAll("\\n", "\n")}</td>
                  </tr>
                  <tr>
                    <td>Response</td>
                    <td>{call.response}</td>
                  </tr>
                  {#if call.user_consented === null}<tr>
                      <td>Approve</td>
                      <td>
                        <form
                          class="inline"
                          onsubmit={async () => {
                            await manageToolCallConsent(message.sequence, call.call_id, true);
                          }}
                        >
                          <button class="btn btn-success"><BadgeCheck /></button>
                        </form>
                        <form
                          class="inline"
                          onsubmit={async () => {
                            await manageToolCallConsent(message.sequence, call.call_id, false);
                          }}
                        >
                          <button class="btn btn-error"><BadgeX /></button>
                        </form>
                      </td>
                    </tr>
                  {/if}
                </tbody>
              </table>
            {/each}
          </div>
        </div>
      {/if}
      {@html DOMPurify.sanitize(marked(message.content) as string)}
    </div>
  </div>
{/each}
<br />
<br />
<br />

<div class="fixed bottom-0 footer footer-center">
  <form
    class="inline"
    onsubmit={async () => {
      await sendMessage();
    }}
  >
    <textarea
      class="textarea w-5xl resize-none"
      placeholder="Send Message"
      disabled={disableSend()}
      required
      bind:value={content}
      onkeydown={async (e) => {
        if (e.key == "Enter" && !e.shiftKey) {
          e.preventDefault();
          await sendMessage();
        }
      }}
    ></textarea>
    <button class="btn btn-primary btn-circle btn-xl" disabled={disableSend()}><SendHorizontal /></button>
  </form>
</div>
