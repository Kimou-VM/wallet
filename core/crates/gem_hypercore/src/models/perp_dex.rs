const HIP3_PERP_ASSET_OFFSET: u32 = 100_000;
const HIP3_PERP_ASSET_STRIDE: u32 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerpDex {
    Main,
    Xyz,
}

impl PerpDex {
    pub fn name(self) -> Option<String> {
        match self {
            Self::Main => None,
            Self::Xyz => Some("xyz".to_string()),
        }
    }

    pub fn is_isolated_only(self) -> bool {
        match self {
            Self::Main => false,
            Self::Xyz => true,
        }
    }

    pub fn asset_index(self, meta_index: u32) -> u32 {
        match self {
            Self::Main => meta_index,
            Self::Xyz => HIP3_PERP_ASSET_OFFSET + HIP3_PERP_ASSET_STRIDE + meta_index,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_index() {
        assert_eq!(PerpDex::Main.asset_index(5), 5);
        assert_eq!(PerpDex::Xyz.asset_index(0), 110_000, "xyz is Hyperliquid's perp dex 1, so its first market is 100000 + 1 * 10000");
        assert_eq!(PerpDex::Xyz.asset_index(3), 110_003);
    }
}
