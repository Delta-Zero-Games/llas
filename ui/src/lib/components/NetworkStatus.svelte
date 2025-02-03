<!-- ui/src/lib/components/NetworkStatus.svelte -->
<script lang="ts">
  import { networkStore } from '../stores/networkStore';

  // Get connection status text and color
  $: statusText = $networkStore.isConnected ? 'Connected' : 'Disconnected';
  $: statusColor = $networkStore.isConnected ? '#3cf281' : '#ef4444';
  
  // Get quality color based on connection quality
  $: qualityColor = (() => {
    if (!$networkStore.isConnected) return '#ef4444';
    
    switch ($networkStore.stats.connectionQuality) {
      case 'Excellent':
        return '#3cf281';
      case 'Good':
        return '#22c55e';
      case 'Fair':
        return '#eab308';
      case 'Poor':
        return '#f97316';
      case 'Critical':
        return '#ef4444';
      default:
        return '#ef4444';
    }
  })();
</script>

<div class="flex items-center gap-2 text-sm">
  <div class="flex items-center gap-1.5">
    <div class="w-2 h-2 rounded-full" style="background-color: {statusColor}"></div>
    <span class="text-zinc-300">{statusText}</span>
  </div>

  {#if $networkStore.isConnected}
    <div class="flex items-center gap-1.5">
      <div class="w-2 h-2 rounded-full" style="background-color: {qualityColor}"></div>
      <span class="text-zinc-300">{$networkStore.stats.connectionQuality}</span>
    </div>
  {/if}

  {#if $networkStore.error}
    <div class="text-red-500">
      {$networkStore.error}
    </div>
  {/if}
</div>