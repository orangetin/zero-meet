<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    count,
    featured = false,
    children,
  }: {
    count: number;
    featured?: boolean;
    children: Snippet;
  } = $props();

  const gap = 13;
  const aspectRatio = 16 / 9;
  let width = $state(0);
  let height = $state(0);

  const layout = $derived.by(() => {
    const cells = count + (featured ? 3 : 0);
    let best = { columns: 1, width: 0, height: 0 };

    // Try each column count and keep the arrangement with the largest tiles.
    // A pinned participant occupies a two-by-two block.
    for (
      let columns = featured ? 2 : 1;
      columns <= Math.max(cells, 1);
      columns++
    ) {
      const rows = Math.max(featured ? 2 : 1, Math.ceil(cells / columns));
      const cellWidth = (width - gap * (columns - 1)) / columns;
      const cellHeight = (height - gap * (rows - 1)) / rows;
      const tileWidth = Math.max(
        0,
        Math.min(cellWidth, cellHeight * aspectRatio),
      );

      if (tileWidth > best.width) {
        best = { columns, width: tileWidth, height: tileWidth / aspectRatio };
      }
    }

    return best;
  });
</script>

<div
  class="participant-grid"
  bind:clientWidth={width}
  bind:clientHeight={height}
  style:grid-template-columns={`repeat(${layout.columns}, ${layout.width}px)`}
  style:grid-auto-rows={`${layout.height}px`}
  style:--row-height={`${layout.height}px`}
  style:--tile-gap={`${gap}px`}
>
  {@render children()}
</div>

<style>
  .participant-grid {
    display: grid;
    flex: 1;
    min-height: 160px;
    min-width: 0;
    gap: var(--tile-gap);
    place-content: center;
    place-items: center;
  }
</style>
