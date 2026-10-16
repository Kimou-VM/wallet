use borsh::BorshDeserialize;

use crate::{Pubkey, SolanaError, error::Result};

const MINT_LEN: usize = 82;
const ACCOUNT_TYPE_OFFSET: usize = 165;
const EXTENSIONS_OFFSET: usize = 166;
const MINT_ACCOUNT_TYPE: u8 = 1;
const TLV_HEADER_LEN: usize = 4;
const TRANSFER_FEES_OFFSET: usize = 72;
const FROZEN_ACCOUNT_STATE: u8 = 2;
const ONE_IN_BASIS_POINTS: u128 = 10_000;

const UNINITIALIZED: u16 = 0;
const TRANSFER_FEE_CONFIG: u16 = 1;
const DEFAULT_ACCOUNT_STATE: u16 = 6;
const NON_TRANSFERABLE: u16 = 9;
const TRANSFER_HOOK: u16 = 14;
const PAUSABLE: u16 = 26;

#[derive(Debug, Clone, Copy, PartialEq, BorshDeserialize)]
pub struct TransferFee {
    pub epoch: u64,
    pub maximum_fee: u64,
    pub basis_points: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, BorshDeserialize)]
pub struct TransferFeeConfig {
    pub older: TransferFee,
    pub newer: TransferFee,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MintExtension {
    TransferFee(TransferFeeConfig),
    TransferHook { program_id: Option<Pubkey> },
    NonTransferable,
    DefaultAccountState { frozen: bool },
    Pausable { paused: bool },
    Other(u16),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Mint {
    pub extensions: Vec<MintExtension>,
}

impl Mint {
    pub fn decode(data: &[u8]) -> Result<Self> {
        match (data.len(), data.get(ACCOUNT_TYPE_OFFSET)) {
            (MINT_LEN, _) => Ok(Self { extensions: vec![] }),
            (_, Some(&MINT_ACCOUNT_TYPE)) => Ok(Self {
                extensions: decode_extensions(&data[EXTENSIONS_OFFSET..])?,
            }),
            _ => Err(SolanaError::invalid_input("Invalid mint account")),
        }
    }

    pub fn transfer_fee(&self, epoch: u64, amount: u64) -> u64 {
        self.extensions
            .iter()
            .find_map(|extension| match extension {
                MintExtension::TransferFee(config) => Some(config.fee(epoch, amount)),
                _ => None,
            })
            .unwrap_or_default()
    }
}

impl TransferFeeConfig {
    fn fee(&self, epoch: u64, amount: u64) -> u64 {
        let fee = if epoch >= self.newer.epoch { self.newer } else { self.older };
        if fee.basis_points == 0 || amount == 0 {
            return 0;
        }
        let raw_fee = (u128::from(amount) * u128::from(fee.basis_points)).div_ceil(ONE_IN_BASIS_POINTS);
        u64::try_from(raw_fee).unwrap_or(u64::MAX).min(fee.maximum_fee)
    }
}

fn decode_extensions(tlv: &[u8]) -> Result<Vec<MintExtension>> {
    let Some((header, rest)) = tlv.split_at_checked(TLV_HEADER_LEN) else {
        return Ok(vec![]);
    };
    let extension_type = u16::from_le_bytes([header[0], header[1]]);
    let length = usize::from(u16::from_le_bytes([header[2], header[3]]));
    if extension_type == UNINITIALIZED {
        return Ok(vec![]);
    }
    let (value, rest) = rest.split_at_checked(length).ok_or_else(|| SolanaError::invalid_input("Invalid mint extension"))?;
    Ok([vec![decode_extension(extension_type, value)?], decode_extensions(rest)?].concat())
}

fn decode_extension(extension_type: u16, value: &[u8]) -> Result<MintExtension> {
    let invalid = || SolanaError::invalid_input("Invalid mint extension");
    match extension_type {
        TRANSFER_FEE_CONFIG => {
            let fees = value.get(TRANSFER_FEES_OFFSET..).ok_or_else(invalid)?;
            Ok(MintExtension::TransferFee(TransferFeeConfig::deserialize(&mut &fees[..]).map_err(|_| invalid())?))
        }
        TRANSFER_HOOK => {
            let program_id = value.get(32..64).ok_or_else(invalid)?;
            Ok(MintExtension::TransferHook {
                program_id: program_id.iter().any(|byte| *byte != 0).then(|| Pubkey::new(program_id.try_into().unwrap_or_default())),
            })
        }
        NON_TRANSFERABLE => Ok(MintExtension::NonTransferable),
        DEFAULT_ACCOUNT_STATE => Ok(MintExtension::DefaultAccountState {
            frozen: *value.first().ok_or_else(invalid)? == FROZEN_ACCOUNT_STATE,
        }),
        PAUSABLE => Ok(MintExtension::Pausable {
            paused: *value.get(32).ok_or_else(invalid)? != 0,
        }),
        other => Ok(MintExtension::Other(other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gem_encoding::decode_base64;

    fn decode(data: &str) -> Mint {
        Mint::decode(&decode_base64(data.trim()).unwrap()).unwrap()
    }

    #[test]
    fn test_decode() {
        let fee = |epoch, basis_points| TransferFee { epoch, maximum_fee: u64::MAX, basis_points };

        assert_eq!(decode(include_str!("../../testdata/mint_usdc.base64")), Mint { extensions: vec![] });
        assert_eq!(
            decode(include_str!("../../testdata/mint_transfer_fee.base64")),
            Mint {
                extensions: vec![MintExtension::TransferFee(TransferFeeConfig {
                    older: fee(1035, 200),
                    newer: fee(1035, 200)
                })],
            }
        );
        assert_eq!(
            decode(include_str!("../../testdata/mint_xstock.base64")),
            Mint {
                extensions: vec![
                    MintExtension::Other(18),
                    MintExtension::Other(12),
                    MintExtension::DefaultAccountState { frozen: false },
                    MintExtension::Other(25),
                    MintExtension::Pausable { paused: false },
                    MintExtension::Other(4),
                    MintExtension::TransferHook { program_id: None },
                    MintExtension::Other(19),
                ],
            }
        );
        assert_eq!(Mint::decode(&[0; 10]), Err(SolanaError::invalid_input("Invalid mint account")));
        assert_eq!(Mint::decode(&[0; 165]), Err(SolanaError::invalid_input("Invalid mint account")));
    }

    #[test]
    fn test_transfer_fee() {
        let flat = decode(include_str!("../../testdata/mint_transfer_fee.base64"));
        let schedule = decode(include_str!("../../testdata/mint_transfer_fee_schedule.base64"));
        let capped = TransferFeeConfig {
            older: TransferFee { epoch: 0, maximum_fee: 5, basis_points: 200 },
            newer: TransferFee { epoch: 0, maximum_fee: 5, basis_points: 200 },
        };

        assert_eq!(flat.transfer_fee(1_059, 1_000_000), 20_000);
        assert_eq!(flat.transfer_fee(1_059, 1), 1);
        assert_eq!(flat.transfer_fee(1_059, 0), 0);
        assert_eq!(schedule.transfer_fee(1_049, 1_000_000), 50_000);
        assert_eq!(schedule.transfer_fee(1_050, 1_000_000), 10_000);
        assert_eq!(capped.fee(1, 1_000_000), 5);
        assert_eq!(decode(include_str!("../../testdata/mint_usdc.base64")).transfer_fee(1_059, 1_000_000), 0);
    }
}
