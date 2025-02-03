export interface User {
    id: string;
    name: string;
    avatarUrl?: string;
    isOnline: boolean;
    lastSeen?: Date;
    status?: 'online' | 'away' | 'busy' | 'offline';
    is_muted: boolean;
    is_deafened: boolean;
    volume?: number;
}
