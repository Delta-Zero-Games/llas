// ui/src/lib/stores/networkStore.ts
import { writable, get } from 'svelte/store';
import type { PeerConnection } from '../types/network';
import { invoke } from '@tauri-apps/api/core';

export interface NetworkStats {
  latency: number;
  packetLoss: number;
  jitter: number;
  bufferSize: number;
  connectionQuality: 'Excellent' | 'Good' | 'Fair' | 'Poor' | 'Critical';
}

export interface NetworkState {
  isConnected: boolean;
  peers: Map<string, PeerConnection>;
  localSessionId: string;
  currentPeer: string | null;
  stats: NetworkStats;
  error: string | null;
}

const initialState: NetworkState = {
  isConnected: false,
  peers: new Map(),
  localSessionId: crypto.randomUUID(),
  currentPeer: null,
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
    
    addPeer: (peerId: string, connection: PeerConnection) =>
      update(state => {
        state.peers.set(peerId, connection);
        return { ...state };
      }),

    removePeer: (peerId: string) =>
      update(state => {
        state.peers.delete(peerId);
        if (state.currentPeer === peerId) {
          state.currentPeer = null;
        }
        return { ...state };
      }),

    setConnected: (isConnected: boolean, peer?: string) =>
      update(state => ({ 
        ...state, 
        isConnected,
        currentPeer: peer || state.currentPeer
      })),

    startMonitoring: async () => {
      try {
        // Listen for network stats from Rust
        await invoke('subscribe_to_network_stats', {}, (event: any) => {
          update(state => ({
            ...state,
            stats: {
              latency: event.latency,
              packetLoss: event.packet_loss,
              jitter: event.jitter,
              bufferSize: event.buffer_size,
              connectionQuality: event.connection_quality
            }
          }));
        });
      } catch (err) {
        console.error('Failed to start network monitoring:', err);
        update(state => ({
          ...state,
          error: err instanceof Error ? err.message : 'Failed to start network monitoring'
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

    reset: () => set(initialState),

    getPeer: (peerId: string) => {
      const state = get(networkStore);
      return state.peers.get(peerId);
    },

    getPeers: () => {
      const state = get(networkStore);
      return Array.from(state.peers.values());
    }
  };
}

export const networkStore = createNetworkStore();

// Subscribe to changes to update connection status
networkStore.subscribe(state => {
  if (state.peers.size === 0 && state.isConnected) {
    networkStore.setConnected(false);
  }
});