// ui/src/lib/stores/audioStore.ts
import { writable } from 'svelte/store';
import { invoke } from '@tauri-apps/api/tauri';

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
        update(state => ({ ...state, isConnected: true }));
      } catch (err) {
        update(state => ({ 
          ...state, 
          error: err instanceof Error ? err.message : 'Failed to start audio'
        }));
      }
    },
    stopAudio: async () => {
      try {
        await invoke('stop_audio');
        update(state => ({ ...state, isConnected: false }));
      } catch (err) {
        update(state => ({ 
          ...state, 
          error: err instanceof Error ? err.message : 'Failed to stop audio'
        }));
      }
    },
    setDevices: async (input: MediaDeviceInfo | null, output: MediaDeviceInfo | null) => {
      try {
        if (input) {
          await invoke('set_input_device', { deviceId: input.deviceId });
        }
        update(state => ({ ...state, inputDevice: input, outputDevice: output }));
      } catch (err) {
        update(state => ({ 
          ...state, 
          error: err instanceof Error ? err.message : 'Failed to set devices'
        }));
      }
    },
    setVolume: async (type: 'input' | 'output', value: number) => {
      try {
        await invoke(`set_${type}_volume`, { volume: value });
        update(state => ({
          ...state,
          [type === 'input' ? 'inputVolume' : 'outputVolume']: value
        }));
      } catch (err) {
        update(state => ({ 
          ...state, 
          error: err instanceof Error ? err.message : 'Failed to set volume'
        }));
      }
    },
    toggleMute: async () => {
      update(state => {
        const newMuted = !state.isMuted;
        invoke('set_muted', { muted: newMuted });
        return { ...state, isMuted: newMuted };
      });
    },
    setUserVolume: async (userId: string, volume: number) => {
        try {
            await invoke('set_user_volume', { userId, volume });
            update(state => ({
            ...state,
            // Could store user volumes here if needed
            }));
        } catch (err) {
            update(state => ({ 
            ...state, 
            error: err instanceof Error ? err.message : 'Failed to set user volume'
            }));
        }
    },
    clearError: () => update(state => ({ ...state, error: null }))
  };
}

export const audioStore = createAudioStore();