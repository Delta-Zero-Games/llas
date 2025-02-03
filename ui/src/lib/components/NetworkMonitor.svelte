<!-- src/lib/components/NetworkMonitor.svelte -->
<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { networkStore, type NetworkStats } from '../stores/networkStore';
    import { audioStore } from '../stores/audioStore';
  
    interface StatsPoint {
      time: string;
      latency: number;
      packetLoss: number;
      jitter: number;
      quality: 'Excellent' | 'Good' | 'Fair' | 'Poor' | 'Critical';
    }
  
    let networkStats: StatsPoint[] = [];
    let currentStats: NetworkStats = $networkStore.stats;
    let audioLevels = {
      input: 0,
      output: 0
    };
  
    function getQualityColor(quality: string): string {
      switch (quality) {
        case 'Excellent': return '#3cf281';
        case 'Good': return '#22c55e';
        case 'Fair': return '#eab308';
        case 'Poor': return '#f97316';
        case 'Critical': return '#ef4444';
        default: return '#71717a';
      }
    }
  
    let interval: ReturnType<typeof setInterval>;
  
    onMount(() => {
      // Start periodic stats updates
      interval = setInterval(() => {
        const newStats: StatsPoint = {
          time: new Date().toISOString(),
          latency: $networkStore.stats.latency,
          packetLoss: $networkStore.stats.packetLoss * 100, // Convert to percentage
          jitter: $networkStore.stats.jitter,
          quality: $networkStore.stats.connectionQuality
        };
        
        networkStats = [...networkStats, newStats].slice(-30); // Keep last 30 samples
        currentStats = $networkStore.stats;
        
        audioLevels = {
          input: $audioStore.inputLevel,
          output: $audioStore.outputVolume
        };
      }, 1000);
    });
  
    onDestroy(() => {
      if (interval) {
        clearInterval(interval);
      }
    });
  </script>
  
  <div class="p-4 bg-zinc-800 rounded-lg space-y-4">
    <div class="flex items-center justify-between">
      <h2 class="text-lg font-semibold text-white">Network Monitor</h2>
      <div class="flex items-center gap-2">
        <span class="text-sm text-zinc-300">Quality:</span>
        <span 
          class="text-sm font-medium px-2 py-1 rounded"
          style="background-color: {getQualityColor(currentStats.connectionQuality)}; 
                 color: {currentStats.connectionQuality === 'Critical' ? 'white' : 'black'}"
        >
          {currentStats.connectionQuality}
        </span>
      </div>
    </div>
    
    <!-- Current Stats -->
    <div class="grid grid-cols-3 gap-4">
      <div class="bg-zinc-700 p-3 rounded">
        <div class="text-sm text-zinc-400">Latency</div>
        <div class="text-xl font-semibold text-white">
          {currentStats.latency.toFixed(1)}ms
        </div>
      </div>
      <div class="bg-zinc-700 p-3 rounded">
        <div class="text-sm text-zinc-400">Packet Loss</div>
        <div class="text-xl font-semibold text-white">
          {(currentStats.packetLoss * 100).toFixed(1)}%
        </div>
      </div>
      <div class="bg-zinc-700 p-3 rounded">
        <div class="text-sm text-zinc-400">Jitter</div>
        <div class="text-xl font-semibold text-white">
          {currentStats.jitter.toFixed(1)}ms
        </div>
      </div>
    </div>
  
    <!-- Audio Levels -->
    <div class="space-y-2">
      <div class="space-y-1">
        <div class="flex justify-between text-sm text-zinc-300">
          <span>Input Level</span>
          <span>{(audioLevels.input * 100).toFixed(0)}%</span>
        </div>
        <div class="w-full h-2 bg-zinc-700 rounded overflow-hidden">
          <div 
            class="h-full bg-blue-500 transition-all duration-100"
            style="width: {audioLevels.input * 100}%"
          ></div>
        </div>
      </div>
  
      <div class="space-y-1">
        <div class="flex justify-between text-sm text-zinc-300">
          <span>Output Level</span>
          <span>{(audioLevels.output * 100).toFixed(0)}%</span>
        </div>
        <div class="w-full h-2 bg-zinc-700 rounded overflow-hidden">
          <div 
            class="h-full bg-green-500 transition-all duration-100"
            style="width: {audioLevels.output * 100}%"
          ></div>
        </div>
      </div>
    </div>
  </div>