<script lang="ts">
  import { onMount } from "svelte";

  let {
    oncancel,
    onconfirm,
  }: {
    oncancel: () => void;
    onconfirm: () => void;
  } = $props();

  let dialog: HTMLDialogElement;

  function cancel() {
    dialog.close();
    oncancel();
  }

  onMount(() => {
    dialog.showModal();

    return () => dialog.close();
  });
</script>

<dialog
  bind:this={dialog}
  aria-labelledby="leave-meeting-title"
  oncancel={cancel}
>
  <h2 id="leave-meeting-title">Leave meeting?</h2>

  <div class="actions">
    <button onclick={cancel}>Cancel</button>
    <button class="leave" onclick={onconfirm}>Leave</button>
  </div>
</dialog>

<style>
  dialog {
    width: min(360px, calc(100vw - 48px));
    padding: 24px;
    color: var(--ink);
    background: var(--paper);
    border: 2px solid var(--ink);
    border-radius: 10px;
    box-shadow: 6px 6px 0 var(--ink);
  }

  dialog::backdrop {
    background: rgb(20 23 18 / 65%);
  }

  h2 {
    margin-bottom: 28px;
    text-align: center;
  }

  .actions {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;
  }

  button {
    width: 100%;
  }
</style>
