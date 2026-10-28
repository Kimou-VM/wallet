use bitcoin::{
    ScriptBuf,
    blockdata::{opcodes::all::OP_RETURN, script::Builder},
    script::PushBytesBuf,
};
use primitives::{BitcoinChain, OutputMinimum, SignerError};

use super::PlanOutput;
use crate::signer::address::script_for_address;

const MAX_OP_RETURN_BYTES: usize = 80;

// OP_RETURN is placed at output index 1 (after the destination): required by Chainflip's
// vault-swap scanner, accepted by Thorchain which scans all outputs.
pub(super) fn spend_outputs(amount: u64, payment_script: ScriptBuf, memo_output: Option<PlanOutput>) -> Vec<PlanOutput> {
    let mut outputs = vec![PlanOutput::new(amount, payment_script)];
    if let Some(output) = memo_output {
        outputs.push(output);
    }
    outputs
}

pub(super) fn op_return_output(data: &[u8]) -> Result<PlanOutput, SignerError> {
    if data.len() > MAX_OP_RETURN_BYTES {
        return SignerError::invalid_input_err("Bitcoin memo is too large");
    }
    let push = PushBytesBuf::try_from(data.to_vec()).map_err(SignerError::from_display)?;
    Ok(PlanOutput::new(0, Builder::new().push_opcode(OP_RETURN).push_slice(push).into_script()))
}

pub(super) fn dust_threshold(chain: BitcoinChain, script_pubkey: &ScriptBuf) -> u64 {
    if script_pubkey.is_op_return() {
        return 0;
    }
    let threshold = script_pubkey.minimal_non_dust().to_sat();
    match chain {
        BitcoinChain::Doge => threshold.saturating_mul(chain.minimum_byte_fee() as u64),
        BitcoinChain::Bitcoin | BitcoinChain::BitcoinCash | BitcoinChain::Litecoin | BitcoinChain::Dash | BitcoinChain::Zcash => threshold,
    }
}

pub(crate) fn output_minimum(chain: BitcoinChain, sender_address: &str, destination_address: &str) -> Result<OutputMinimum, SignerError> {
    Ok(OutputMinimum {
        payment: dust_threshold(chain, &script_for_address(chain, destination_address)?.script_pubkey),
        change: dust_threshold(chain, &script_for_address(chain, sender_address)?.script_pubkey),
    })
}
