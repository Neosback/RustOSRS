export const enum FriendsChatRank {
    Unranked = -1,
    Friend = 0,
    Recruit = 1,
    Corporal = 2,
    Sergeant = 3,
    Lieutenant = 4,
    Captain = 5,
    General = 6,
    Owner = 7,
    JagexModerator = 127,
}

export const enum FriendsChatActionCode {
    Join = 0,
    Leave = 1,
    Kick = 2,
    AddFriend = 3,
    RemoveFriend = 4,
    SetFriendRank = 5,
    AddIgnore = 6,
    RemoveIgnore = 7,
}

export type FriendsChatAction =
    | { action: "join"; name: string }
    | { action: "leave" }
    | { action: "kick"; name: string }
    | { action: "add_friend"; name: string }
    | { action: "remove_friend"; name: string }
    | { action: "set_friend_rank"; name: string; rank: number }
    | { action: "add_ignore"; name: string }
    | { action: "remove_ignore"; name: string };

export interface FriendsChatMemberSnapshot {
    name: string;
    world: number;
    rank: number;
}

export interface FriendSnapshot extends FriendsChatMemberSnapshot {
    previousName: string;
    isOnline: boolean;
}

export interface IgnoreSnapshot {
    name: string;
    previousName: string;
}

export interface FriendsChatSnapshot {
    channel?: {
        name: string;
        owner: string;
        minKickRank: number;
        localRank: number;
        members: FriendsChatMemberSnapshot[];
    };
    friends: FriendSnapshot[];
    ignores: IgnoreSnapshot[];
}
