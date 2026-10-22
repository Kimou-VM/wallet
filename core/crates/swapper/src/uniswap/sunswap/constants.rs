use gem_evm::uniswap::FeeTier;

pub(super) const QUOTER: &str = "TLhZ48yfHygMLM2uZr87zJJusHjGen97gh";
pub(super) const FACTORY: &str = "TThJt8zaJzJMhCEScH7zWKnp5buVZqys9x";

pub(super) const FEE_TIERS: [FeeTier; 4] = [FeeTier::Hundred, FeeTier::FiveHundred, FeeTier::ThreeThousand, FeeTier::TenThousand];
pub(super) const SWAP_ENERGY_LIMIT: u64 = 600_000;
pub(super) const REFERRAL_FEE_BPS: u32 = 85;
