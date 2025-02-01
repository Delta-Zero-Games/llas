<!-- ui/src/lib/components/AudioMeter.svelte -->
<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { audioStore } from '../stores/audioStore';
  
    export let stream: MediaStream | null = null;
    
    let audioContext: AudioContext | null = null;
    let analyser: AnalyserNode | null = null;
    let dataArray: Uint8Array;
    let animationFrame: number;
  
    onMount(() => {
      if (!stream) return;
      
      audioContext = new AudioContext();
      analyser = audioContext.createAnalyser();
      analyser.fftSize = 256;
      
      const source = audioContext.createMediaStreamSource(stream);
      source.connect(analyser);
      
      dataArray = new Uint8Array(analyser.frequencyBinCount);
      
      function updateMeter() {
        if (!analyser) return;
        
        analyser.getByteFrequencyData(dataArray);
        const average = dataArray.reduce((acc, val) => acc + val, 0) / dataArray.length;
        const level = average / 255; // Normalize to 0-1
        
        audioStore.setInputLevel(level);
        animationFrame = requestAnimationFrame(updateMeter);
      }
      
      updateMeter();
    });
  
    onDestroy(() => {
      if (animationFrame) cancelAnimationFrame(animationFrame);
      if (audioContext) audioContext.close();
    });
  
    $: meterHeight = $audioStore.inputLevel * 100;
  </script>
  
  <div class="w-full h-2 bg-gray-700 rounded overflow-hidden">
    <div 
      class="h-full bg-blue-500 transition-all duration-100"
      style="width: {meterHeight}%"
    />
  </div>