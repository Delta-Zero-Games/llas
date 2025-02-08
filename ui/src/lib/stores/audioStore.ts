// ui/src/lib/stores/audioStore.ts
import { writable } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { listen, type Event } from '@tauri-apps/api/event';

export interface AudioState {
  inputDevice: MediaDeviceInfo | null;
  outputDevice: MediaDeviceInfo | null;
  isConnected: boolean;
  inputVolume: number;
  outputVolume: number;
  isMuted: boolean;
  isDeafened: boolean;
  inputLevel: number;
  error: string | null;
}

const initialState: AudioState = {
  inputDevice: null,
  outputDevice: null,
  isConnected: false,
  inputVolume: 1,
  outputVolume: 1,
  isMuted: false,
  isDeafened: false,
  inputLevel: 0,
  error: null
};

interface AudioStatusEvent {
  connected: boolean;
}

interface AudioErrorEvent {
  error: string;
}

function createAudioStore() {
  const { subscribe, set, update } = writable<AudioState>(initialState);

  return {
    subscribe,
    
    startStreaming: async (roomId: string) => {
      try {
        await invoke('start_streaming', { roomId });
        update(state => ({ 
          ...state, 
          isConnected: true, 
          error: null 
        }));
      } catch (err) {
        update(state => ({
          ...state,
          isConnected: false,
          error: err instanceof Error ? err.message : 'Failed to start streaming'
        }));
        throw err;
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

    // Add event listener setup
    initialize: async () => {
      try {
        // Listen for audio connection status events
        await listen<AudioStatusEvent>('audio:status', (event: Event<AudioStatusEvent>) => {
          update(state => ({
            ...state,
            isConnected: event.payload.connected
          }));
        });

        // Listen for audio errors
        await listen<AudioErrorEvent>('audio:error', (event: Event<AudioErrorEvent>) => {
          update(state => ({
            ...state,
            error: event.payload.error
          }));
        });
      } catch (error) {
        console.error('Failed to initialize audio event listeners:', error);
      }
    },

    setInputDevice: async (deviceId: string) => {
      try {
        await invoke('set_input_device', { deviceId });
        update(state => ({ ...state, error: null }));
      } catch (err) {
        update(state => ({ ...state, error: err instanceof Error ? err.message : 'Failed to set input device' }));
      }
    },

    setInputVolume: async (volume: number) => {
      try {
        await invoke('set_input_volume', { volume });
        update(state => ({ ...state, inputVolume: volume, error: null }));
      } catch (err) {
        update(state => ({ ...state, error: err instanceof Error ? err.message : 'Failed to set input volume' }));
      }
    },

    setUserVolume: async (userId: string, volume: number) => {
      try {
        await invoke('set_user_volume', { userId, volume });
        update(state => ({ ...state, outputVolume: volume, error: null }));
      } catch (err) {
        update(state => ({ ...state, error: err instanceof Error ? err.message : 'Failed to set user volume' }));
      }
    },

    toggleMute: async () => {
      update(state => {
        const newMuted = !state.isMuted;
        invoke('set_muted', { muted: newMuted })
          .catch(err => {
            state.error = err instanceof Error ? err.message : 'Failed to set mute state';
          });
        return { ...state, isMuted: newMuted };
      });
    },

    toggleDeafen: async () => {
      update(state => {
        const newDeafened = !state.isDeafened;
        // Set output volume to 0 when deafened, restore when undeafened
        invoke('set_user_volume', { userId: 'global', volume: newDeafened ? 0 : state.outputVolume })
          .catch(err => {
            state.error = err instanceof Error ? err.message : 'Failed to set deafen state';
          });
        return { ...state, isDeafened: newDeafened };
      });
    },

    updateInputLevel: (level: number) => {
      update(state => ({ ...state, inputLevel: level }));
    },

    reset: () => {
      set(initialState);
    }
  };
}

export const audioStore = createAudioStore();