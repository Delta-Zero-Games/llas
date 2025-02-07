// ui/src/lib/stores/networkStore.ts
import { writable } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export interface NetworkStats {
  latency: number;
  packetLoss: number;
  jitter: number;
  bufferSize: number;
  connectionQuality: 'Excellent' | 'Good' | 'Fair' | 'Poor' | 'Critical';
}

interface RustDuration {
  secs: number;
  nanos: number;
}

interface NetworkStatsEvent {
  peer: string;
  stats: {
    latency: RustDuration;
    packet_loss: number;
    jitter: RustDuration;
    connection_quality: 'Excellent' | 'Good' | 'Fair' | 'Poor' | 'Critical';
  };
}

export interface NetworkState {
  isConnected: boolean;
  currentRoomId: string | null;
  stats: NetworkStats;
  error: string | null;
}

const initialState: NetworkState = {
  isConnected: false,
  currentRoomId: null,
  stats: {
    latency: 0,
    packetLoss: 0,
    jitter: 0,
    bufferSize: 0,
    connectionQuality: 'Good'
  },
  error: null
};

function createNetworkStore() {
  const { subscribe, set, update } = writable<NetworkState>(initialState);
  let eventUnsubscribers: (() => void)[] = [];

  // Helper to convert Rust Duration to milliseconds
  const durationToMs = (duration: RustDuration): number => {
    return duration.secs * 1000 + duration.nanos / 1_000_000;
  };

  // Set up event listeners
  async function setupEventListeners() {
    try {
      // Network stats listener
      const statsUnsubscribe = await listen<NetworkStatsEvent>('network:stats', (event) => {
        const { stats } = event.payload;
        update(state => ({
          ...state,
          stats: {
            ...state.stats,
            latency: durationToMs(stats.latency),
            jitter: durationToMs(stats.jitter),
            packetLoss: stats.packet_loss,
            connectionQuality: stats.connection_quality,
          }
        }));
      });
      eventUnsubscribers.push(statsUnsubscribe);

      // Connection status listener
      const statusUnsubscribe = await listen<{ connected: boolean }>('network:status', (event) => {
        update(state => ({
          ...state,
          isConnected: event.payload.connected
        }));
      });
      eventUnsubscribers.push(statusUnsubscribe);

      // Error listener
      const errorUnsubscribe = await listen<{ error: string }>('network:error', (event) => {
        update(state => ({
          ...state,
          error: event.payload.error
        }));
      });
      eventUnsubscribers.push(errorUnsubscribe);

    } catch (error) {
      console.error('Failed to setup network event listeners:', error);
    }
  }

  // Clean up function
  function cleanup() {
    eventUnsubscribers.forEach(unsubscribe => unsubscribe());
    eventUnsubscribers = [];
  }

  // Initialize listeners
  setupEventListeners();

  return {
    subscribe,
    
    setConnected: (isConnected: boolean, roomId?: string) =>
      update(state => ({ 
        ...state, 
        isConnected,
        currentRoomId: roomId || state.currentRoomId
      })),

    startStreaming: async (roomId: string) => {
      try {
        await invoke('start_streaming', { roomId });
        update(state => ({
          ...state,
          isConnected: true,
          currentRoomId: roomId,
          error: null
        }));
      } catch (error) {
        console.error('Failed to start streaming:', error);
        update(state => ({
          ...state,
          isConnected: false,
          error: error instanceof Error ? error.message : 'Failed to start streaming'
        }));
        throw error;
      }
    },

    stopStreaming: async () => {
      try {
        await invoke('stop_streaming');
        update(state => ({
          ...state,
          isConnected: false,
          currentRoomId: null,
          error: null
        }));
      } catch (err) {
        console.error('Failed to stop streaming:', err);
        update(state => ({
          ...state,
          error: err instanceof Error ? err.message : 'Failed to stop streaming'
        }));
      }
    },

    updateStats: (newStats: Partial<NetworkStats>) =>
      update(state => ({
        ...state,
        stats: { ...state.stats, ...newStats }
      })),

    setError: (error: string | null) =>
      update(state => ({ ...state, error })),

    reset: () => {
      cleanup();
      set(initialState);
    },

    initialize: async () => {
      cleanup();
      await setupEventListeners();
    }
  };
}

export const networkStore = createNetworkStore();