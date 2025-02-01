// ui/src/lib/types/network.ts
export interface PeerConnection {
    id: string;
    connection: RTCPeerConnection;
    audioTrack: MediaStreamTrack | null;
    dataChannel: RTCDataChannel | null;
  }
  
  export interface TurnConfig {
    urls: string[];
    username: string;
    credential: string;
  }
  
  export interface NetworkState {
    isConnected: boolean;
    peers: Map<string, PeerConnection>;
    localSessionId: string;
    currentPeer: string | null;
    stats: NetworkStats;
    error: string | null;
  }
  
  export interface NetworkStats {
    latency: number;
    packetLoss: number;
    jitter: number;
    bufferSize: number;
    connectionQuality: 'Excellent' | 'Good' | 'Fair' | 'Poor' | 'Critical';
  }