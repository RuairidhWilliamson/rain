pub type RainHashState = std::hash::DefaultHasher;

pub trait RainHash {
    fn rain_hash(&self, state: &mut dyn std::hash::Hasher);
}

impl<T: std::hash::Hash> RainHash for T {
    fn rain_hash(&self, mut state: &mut dyn std::hash::Hasher) {
        std::hash::Hash::hash(self, &mut state);
    }
}
