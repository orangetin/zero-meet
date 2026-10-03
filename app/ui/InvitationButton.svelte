<script lang="ts">
  import { onDestroy } from "svelte";
  import { Check, Copy } from "lucide-svelte";

  let { invitation }: { invitation: string } = $props();

  let copied = $state(false);
  let copyFailed = $state(false);
  let disposed = false;
  let resetTimer: ReturnType<typeof setTimeout>;

  async function copyInvitation() {
    try {
      await navigator.clipboard.writeText(invitation);
      if (disposed) return;

      copied = true;

      clearTimeout(resetTimer);
      resetTimer = setTimeout(() => {
        copied = false;
      }, 2500);
    } catch {
      if (disposed) return;

      copyFailed = true;
    }
  }

  onDestroy(() => {
    disposed = true;
    clearTimeout(resetTimer);
  });
</script>

<div class="invitation">
  {#if copyFailed}
    <label for="copy-invitation">Select the link to copy it manually</label>
    <input
      id="copy-invitation"
      readonly
      value={invitation}
      onclick={(event) => event.currentTarget.select()}
    />
  {:else}
    <button title="Copy invitation" aria-live="polite" onclick={copyInvitation}>
      {#if copied}
        <Check size={18} />
        Copied!
      {:else}
        <Copy size={18} />
        Invite
      {/if}
    </button>
  {/if}
</div>

<style>
  .invitation {
    min-width: 0;
    justify-self: start;
  }

  button {
    background: #fffef9;
  }

  label {
    font-size: 12px;
  }
</style>
