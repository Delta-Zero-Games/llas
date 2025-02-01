<!-- ui/src/lib/components/NetworkStatus.svelte -->
<script lang="ts">
    import { networkStore } from '../stores/networkStore';
    import { Signal, Wifi, WifiOff } from 'lucide-svelte';
  
    // Map connection quality to colors and icons
    const qualityConfig = {
      Excellent: {
        color: 'bg-green-500',
        signal: 4
      },
      Good: {
        color: 'bg-blue-500',
        signal: 3
      },
      Fair: {
        color: 'bg-yellow-500',
        signal: 2
      },
      Poor: {
        color: 'bg-orange-500',
        signal: 1
      },
      Critical: {
        color: 'bg-red-500',
        signal: 0
      }
    };
  </script>
  
  <div class="space-y-2 p-4 bg-gray-800 rounded-lg">
    <div class="flex items-center justify-between">
      <span class="text-sm font-medium text-gray-300">Network Status</span>
      {#if $networkStore.isConnected}
        <Wifi class="w-4 h-4 text-green-500" />
      {:else}
        <WifiOff class="w-4 h-4 text-red-500" />
      {/if}
    </div>
  
    {#if $networkStore.currentPeer}
      <div class="space-y-1">
        <!-- Latency -->
        <div class="flex items-center justify-between text-sm">
          <span class="text-gray-400">Latency:</span>
          <span class="text-white">{$networkStore.stats.latency}ms</span>
        </div>
  
        <!-- Packet Loss -->
        <div class="flex items-center justify-between text-sm">
          <span class="text-gray-400">Packet Loss:</span>
          <span class="text-white">{($networkStore.stats.packetLoss * 100).toFixed(1)}%</span>
        </div>
  
        <!-- Connection Quality -->
        <div class="flex items-center justify-between text-sm">
          <span class="text-gray-400">Quality:</span>
          <div class="flex items-center gap-1">
            {#each Array(4) as _, i}
              <div 
                class={`w-1 h-3 rounded-sm ${
                  i < qualityConfig[$networkStore.stats.connectionQuality].signal
                  ? qualityConfig[$networkStore.stats.connectionQuality].color
                  : 'bg-gray-600'
                }`}
              />
            {/each}
          </div>
        </div>
  
        <!-- Detailed Stats -->
        <div class="mt-2 p-2 bg-gray-700 rounded text-xs">
          <div class="grid grid-cols-2 gap-2">
            <div>
              <span class="text-gray-400">Jitter:</span>
              <span class="text-white ml-1">{$networkStore.stats.jitter}ms</span>
            </div>
            <div>
              <span class="text-gray-400">Buffer:</span>
              <span class="text-white ml-1">{$networkStore.stats.bufferSize}ms</span>
            </div>
          </div>
        </div>
      </div>
    {:else}
      <div class="text-sm text-gray-500 text-center py-2">
        Not connected to any peers
      </div>
    {/if}
  </div>