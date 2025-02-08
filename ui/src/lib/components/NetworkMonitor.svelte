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

  function getQualityDescription(quality: string): string {
    switch (quality) {
      case 'Excellent': return 'Optimal connection';
      case 'Good': return 'Good connection';
      case 'Fair': return 'Acceptable connection';
      case 'Poor': return 'Connection issues';
      case 'Critical': return 'Severe connection problems';
      default: return 'Unknown quality';
    }
  }

  let interval: ReturnType<typeof setInterval>;

  // Calculate maximum values for scaling
  $: maxLatency = Math.max(...networkStats.map(s => s.latency), 200); // minimum 200ms scale
  $: maxPacketLoss = Math.max(...networkStats.map(s => s.packetLoss), 10); // minimum 10% scale
  $: maxJitter = Math.max(...networkStats.map(s => s.jitter), 50); // minimum 50ms scale

  // Calculate points for line graphs
  $: latencyPoints = networkStats.map((stat, i) => ({
    x: (i / (networkStats.length - 1)) * 100,
    y: 100 - (stat.latency / maxLatency * 100)
  }));

  $: packetLossPoints = networkStats.map((stat, i) => ({
    x: (i / (networkStats.length - 1)) * 100,
    y: 100 - (stat.packetLoss / maxPacketLoss * 100)
  }));

  $: jitterPoints = networkStats.map((stat, i) => ({
    x: (i / (networkStats.length - 1)) * 100,
    y: 100 - (stat.jitter / maxJitter * 100)
  }));

  function getPathFromPoints(points: Array<{x: number, y: number}>): string {
    if (points.length < 2) return '';
    return points.reduce((path, point, i) => 
      path + (i === 0 ? `M ${point.x},${point.y}` : ` L ${point.x},${point.y}`), 
    '');
  }

  onMount(() => {
    interval = setInterval(() => {
      const newStats: StatsPoint = {
        time: new Date().toISOString(),
        latency: $networkStore.stats.latency,
        packetLoss: $networkStore.stats.packetLoss * 100,
        jitter: $networkStore.stats.jitter,
        quality: $networkStore.stats.connectionQuality
      };
      
      networkStats = [...networkStats, newStats].slice(-60);
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
      <div class="flex flex-col items-end">
        <span 
          class="text-sm font-medium px-2 py-1 rounded"
          style="background-color: {getQualityColor(currentStats.connectionQuality)}; 
                 color: {currentStats.connectionQuality === 'Critical' ? 'white' : 'black'}"
        >
          {currentStats.connectionQuality}
        </span>
        <span class="text-xs text-zinc-400 mt-1">
          {getQualityDescription(currentStats.connectionQuality)}
        </span>
      </div>
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

  <!-- Network Stats Graph -->
  {#if networkStats.length > 1}
    <div class="relative h-48 w-full bg-zinc-700/50 rounded overflow-hidden">
      <svg class="w-full h-full" viewBox="0 0 100 100" preserveAspectRatio="none">
        <!-- Grid lines -->
        <path d="M 0,25 H 100 M 0,50 H 100 M 0,75 H 100" 
              stroke="#3f3f46" stroke-width="0.2"/>
        
        <!-- Lines -->
        <path d={getPathFromPoints(latencyPoints)} 
              stroke="#3b82f6" stroke-width="0.5" fill="none"/>
        <path d={getPathFromPoints(packetLossPoints)} 
              stroke="#ef4444" stroke-width="0.5" fill="none"/>
        <path d={getPathFromPoints(jitterPoints)} 
              stroke="#22c55e" stroke-width="0.5" fill="none"/>
      </svg>

      <!-- Legend -->
      <div class="absolute bottom-2 right-2 flex gap-3 text-xs bg-zinc-800/80 p-2 rounded">
        <div class="flex items-center gap-1">
          <div class="w-3 h-0.5 bg-[#3b82f6]"></div>
          <span class="text-zinc-300">Latency</span>
        </div>
        <div class="flex items-center gap-1">
          <div class="w-3 h-0.5 bg-[#ef4444]"></div>
          <span class="text-zinc-300">Packet Loss</span>
        </div>
        <div class="flex items-center gap-1">
          <div class="w-3 h-0.5 bg-[#22c55e]"></div>
          <span class="text-zinc-300">Jitter</span>
        </div>
      </div>
    </div>
  {/if}

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