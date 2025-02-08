<!-- ui/src/lib/components/NetworkStatus.svelte -->
<script lang="ts">
  import { networkStore } from '../stores/networkStore';
  import { ChevronDown } from 'lucide-svelte';
  import { fade } from 'svelte/transition';
  
  let showDetails = false;

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

  // Format latency for display
  $: latencyText = (() => {
    if (!$networkStore.isConnected) return '';
    const ms = $networkStore.stats.latency;
    return `${ms.toFixed(0)}ms`;
  })();

  // Format packet loss for display
  $: packetLossText = (() => {
    if (!$networkStore.isConnected) return '';
    const loss = $networkStore.stats.packetLoss * 100;
    return `${loss.toFixed(1)}%`;
  })();
</script>

<div class="relative">
  <!-- Main Status Display -->
  <button 
    class="flex items-center gap-2 text-sm bg-zinc-800 px-3 py-1.5 rounded-lg hover:bg-zinc-700 transition-colors"
    on:click={() => showDetails = !showDetails}
  >
    <div class="flex items-center gap-1.5">
      <div class="w-2 h-2 rounded-full" style="background-color: {statusColor}"></div>
      <span class="text-zinc-300">{statusText}</span>
    </div>

    {#if $networkStore.isConnected}
      <div class="flex items-center gap-1.5">
        <div class="w-2 h-2 rounded-full" style="background-color: {qualityColor}"></div>
        <span class="text-zinc-300">{$networkStore.stats.connectionQuality}</span>
      </div>

      <div class="text-zinc-400 text-xs">
        {latencyText}
      </div>

      <ChevronDown 
        size={16} 
        class="text-zinc-400 transition-transform" 
        style={showDetails ? 'transform: rotate(180deg)' : ''}
      />
    {/if}
  </button>

  <!-- Detailed Stats Popup -->
  {#if showDetails && $networkStore.isConnected}
    <div 
      class="absolute top-full left-0 mt-2 w-48 bg-zinc-800 rounded-lg shadow-lg p-3 space-y-2 z-50"
      transition:fade
    >
      <div class="space-y-1">
        <div class="flex justify-between items-center">
          <span class="text-zinc-400 text-xs">Latency</span>
          <span class="text-zinc-300 text-xs font-medium">{latencyText}</span>
        </div>
        <div class="flex justify-between items-center">
          <span class="text-zinc-400 text-xs">Packet Loss</span>
          <span class="text-zinc-300 text-xs font-medium">{packetLossText}</span>
        </div>
        <div class="flex justify-between items-center">
          <span class="text-zinc-400 text-xs">Jitter</span>
          <span class="text-zinc-300 text-xs font-medium">
            {$networkStore.stats.jitter.toFixed(1)}ms
          </span>
        </div>
      </div>

      <div class="pt-2 border-t border-zinc-700">
        <div class="flex items-center gap-1.5">
          <div class="w-2 h-2 rounded-full" style="background-color: {qualityColor}"></div>
          <span class="text-zinc-300 text-xs">{$networkStore.stats.connectionQuality}</span>
        </div>
      </div>
    </div>
  {/if}

  <!-- Error Display -->
  {#if $networkStore.error}
    <div class="absolute top-full left-0 mt-2 p-2 bg-red-500/10 border border-red-500 rounded text-red-500 text-xs">
      {$networkStore.error}
    </div>
  {/if}
</div>

<style>
  /* Add click-away functionality for details popup */
  :global(body.details-visible) {
    cursor: pointer;
  }
</style>