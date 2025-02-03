// ui/src/lib/stores/audioStore.ts
import { writable } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';

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

function createAudioStore() {
  const { subscribe, set, update } = writable<AudioState>(initialState);

  return {
    subscribe,
    
    startAudio: async (serverAddr: string) => {
      try {
        await invoke('start_audio', { serverAddr });
        update(state => ({ ...state, isConnected: true, error: null }));
      } catch (err) {
        update(state => ({ ...state, error: err instanceof Error ? err.message : 'Failed to start audio' }));
      }
    },

    setDevices: (input: MediaDeviceInfo | null, output: MediaDeviceInfo | null) => {
      update(state => ({
        ...state,
        inputDevice: input || state.inputDevice,
        outputDevice: output || state.outputDevice
      }));
    },

    setVolume: (type: 'input' | 'output', volume: number) => {
      update(state => ({
        ...state,
        [type === 'input' ? 'inputVolume' : 'outputVolume']: volume
      }));
    },

    setUserVolume: async (userId: string, volume: number) => {
      try {
        await invoke('set_user_volume', { userId, volume });
      } catch (err) {
        update(state => ({ 
          ...state, 
          error: err instanceof Error ? err.message : 'Failed to set user volume'
        }));
      }
    },

    toggleMute: () => {
      update(state => ({ ...state, isMuted: !state.isMuted }));
    },

    setDeafened: (deafened: boolean) => {
      update(state => ({ ...state, isDeafened: deafened }));
    },

    setInputLevel: (level: number) => {
      update(state => ({ ...state, inputLevel: level }));
    },

    clearError: () => {
      update(state => ({ ...state, error: null }));
    }
  };
}

export const audioStore = createAudioStore();