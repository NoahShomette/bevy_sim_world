use crate::player::{Player, PlayerMarker};

use super::{SimComponentId, SaveId};

impl SaveId for PlayerMarker {
    fn save_id(&self) -> SimComponentId {
        0
    }

    fn save_id_const() -> SimComponentId
    where
        Self: Sized,
    {
        0
    }

    #[doc = r" Serializes the state of the object at the given tick into binary. Only saves the keyframe and not the curve itself"]
    fn to_binary(&self) -> Option<Vec<u8>> {
        bincode::serialize(self).ok()
    }
}

impl SaveId for Player {
    fn save_id(&self) -> SimComponentId {
        1
    }

    fn save_id_const() -> SimComponentId
    where
        Self: Sized,
    {
        1
    }

    #[doc = r" Serializes the state of the object at the given tick into binary. Only saves the keyframe and not the curve itself"]
    fn to_binary(&self) -> Option<Vec<u8>> {
        bincode::serialize(self).ok()
    }
}
