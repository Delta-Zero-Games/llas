// ui/src/lib/stores/networkStore.ts
import { writable, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export interface NetworkStats {
  latency: number;
  packetLoss: number;
  jitter: number;
  bufferSize: number;
  connectionQuality: 'Excellent' | 'Good' | 'Fair' | 'Poor' | 'Critical';
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
        throw error; // Re-throw to let components handle it
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

    reset: () => set(initialState)
  };
}

export const networkStore = createNetworkStore();