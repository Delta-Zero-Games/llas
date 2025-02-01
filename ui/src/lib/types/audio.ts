// ui/src/lib/types/audio.ts
export interface AudioDevice {
    id: string;
    name: string;
    type: 'input' | 'output';
  }
  
  export interface AudioState {
    inputDevice: AudioDevice | null;
    outputDevice: AudioDevice | null;
    inputVolume: number;
    outputVolume: number;
    isMuted: boolean;
    isDeafened: boolean;
  }