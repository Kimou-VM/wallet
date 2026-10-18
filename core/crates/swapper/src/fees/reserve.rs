use num_bigint::BigUint;
use primitives::{Chain, ChainType};

pub fn reserved_transaction_fees(chain: Chain) -> Option<BigUint> {
    let reserve: u64 = match chain {
        Chain::Ethereum => 1_000_000_000_000_000,    // 0.001 ETH
        Chain::Arbitrum => 300_000_000_000_000,      // 0.0003 ARB ETH
        Chain::Base => 300_000_000_000_000,          // 0.0003 BASE ETH
        Chain::Optimism => 500_000_000_000_000,      // 0.0005 OP ETH
        Chain::AvalancheC => 3_000_000_000_000_000,  // 0.003 AVAX
        Chain::SmartChain => 2_000_000_000_000_000,  // 0.002 BNB
        Chain::Polygon => 20_000_000_000_000_000,    // 0.02 MATIC
        Chain::Gnosis => 5_000_000_000_000_000,      // 0.005 XDAI
        Chain::Berachain => 5_000_000_000_000_000,   // 0.005 BERA
        Chain::Sui => 50_000_000,                    // 0.05 SUI
        Chain::Solana => 5_000_000,                  // 0.005 SOL: base + priority fees + wSOL ATA rent
        Chain::Tron => 2_000_000,                    // 2 TRX: native Chainflip memo transfer fee plus buffer
        Chain::Ton => 20_000_000,                    // 0.02 TON
        Chain::Aptos => 20_000_000,                  // 0.2 APT
        Chain::Monad => 5_000_000_000_000_000,       // 0.005 MON
        Chain::XLayer => 5_000_000_000_000_000,      // 0.005 OKB
        Chain::Plasma => 5_000_000_000_000_000,      // 0.005 XPL
        Chain::OpBNB => 100_000_000_000_000,         // 0.0001 BNB
        Chain::Mantle => 50_000_000_000_000_000,     // 0.05 MNT
        Chain::Linea => 300_000_000_000_000,         // 0.0003 ETH
        Chain::Celo => 10_000_000_000_000_000,       // 0.01 CELO
        Chain::Sonic => 50_000_000_000_000_000,      // 0.05 S
        Chain::Robinhood => 100_000_000_000_000,     // 0.0001 ETH
        Chain::Hyperliquid => 1_000_000_000_000_000, // 0.001 HYPE
        Chain::Stable => 10_000_000_000_000_000,     // 0.01 USDT0
        _ => return None,
    };
    Some(BigUint::from(reserve))
}

pub fn max_amount_spends_all_but_fee(chain: Chain) -> bool {
    match chain.chain_type() {
        ChainType::Ethereum => true,
        ChainType::Bitcoin
        | ChainType::Solana
        | ChainType::Cosmos
        | ChainType::Ton
        | ChainType::Tron
        | ChainType::Aptos
        | ChainType::Sui
        | ChainType::Xrp
        | ChainType::Near
        | ChainType::Stellar
        | ChainType::Algorand
        | ChainType::Polkadot
        | ChainType::Cardano
        | ChainType::HyperCore => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solana_reserve_covers_rent_and_priority_fees() {
        let reserve = reserved_transaction_fees(Chain::Solana).unwrap();
        assert!(reserve >= BigUint::from(2_500_000u64), "Solana reserve {reserve} too small to cover wSOL ATA rent plus priority fees");
    }

    #[test]
    fn tron_reserve_covers_native_chainflip_memo_transfer() {
        let reserve = reserved_transaction_fees(Chain::Tron).unwrap();
        assert!(reserve >= BigUint::from(1_398_000u64 * 120 / 100));
    }
}
