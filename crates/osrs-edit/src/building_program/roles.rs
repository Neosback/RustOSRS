//! One vocabulary for authored room intent and kit furnishing labels.
use super::BuildingRoomRole;

impl BuildingRoomRole {
    /// Canonical portable label used by room programs and furnishing kits.
    pub const fn label(self) -> &'static str {
        use BuildingRoomRole as R;
        match self {
            R::Living => "living",
            R::Kitchen => "kitchen",
            R::Bedroom => "bedroom",
            R::Bathroom => "bathroom",
            R::Utility => "utility",
            R::SalesFloor => "sales_floor",
            R::Storage => "storage",
            R::Office => "office",
            R::Workshop => "workshop",
            R::CommonRoom => "common_room",
            R::Bar => "bar",
            R::GuestRoom => "guest_room",
            R::Lobby => "lobby",
            R::Vault => "vault",
            R::Security => "security",
            R::Nave => "nave",
            R::Altar => "altar",
            R::Vestry => "vestry",
            R::Loading => "loading",
            R::GreatHall => "great_hall",
            R::Armory => "armory",
            R::GuardRoom => "guard_room",
            R::Cell => "cell",
        }
    }

    /// Resolve a canonical label or a census alias. Unknown/general evidence stays unresolved.
    pub fn from_label(label: &str) -> Option<Self> {
        Some(match label.trim().to_ascii_lowercase().as_str() {
            "living" => Self::Living,
            "kitchen" => Self::Kitchen,
            "bedroom" => Self::Bedroom,
            "bathroom" => Self::Bathroom,
            "utility" => Self::Utility,
            "sales_floor" => Self::SalesFloor,
            "storage" => Self::Storage,
            "office" => Self::Office,
            "workshop" => Self::Workshop,
            "common_room" => Self::CommonRoom,
            "bar" => Self::Bar,
            "guest_room" => Self::GuestRoom,
            "lobby" => Self::Lobby,
            "vault" => Self::Vault,
            "security" => Self::Security,
            "nave" => Self::Nave,
            "altar" => Self::Altar,
            "vestry" => Self::Vestry,
            "loading" => Self::Loading,
            "great_hall" => Self::GreatHall,
            "armory" => Self::Armory,
            "guard_room" => Self::GuardRoom,
            "cell" => Self::Cell,
            "smithy" => Self::Workshop,
            "bank" => Self::Lobby,
            "pub" | "tavern" => Self::CommonRoom,
            "shop" => Self::SalesFloor,
            "chapel" | "church" => Self::Nave,
            "study" => Self::Office,
            _ => return None,
        })
    }
}
