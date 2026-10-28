use std::error::Error;

use primitives::{BitcoinChain, OutputMinimum, SignerError, TransactionLoadMetadata, TransactionPreloadInput, UTXO};

use crate::models::{BitcoinNodeInfo, BitcoinUTXO};
use crate::signer::output_minimum;

pub fn map_transaction_preload(chain: BitcoinChain, utxos: Vec<BitcoinUTXO>, input: TransactionPreloadInput) -> Result<TransactionLoadMetadata, SignerError> {
    let output_minimum = preload_output_minimum(chain, &input)?;
    let utxos = map_utxos(utxos, input.sender_address);
    Ok(TransactionLoadMetadata::Bitcoin { utxos, output_minimum })
}

fn preload_output_minimum(chain: BitcoinChain, input: &TransactionPreloadInput) -> Result<OutputMinimum, SignerError> {
    output_minimum(chain, &input.sender_address, input.input_type.swap_to_address().unwrap_or(&input.destination_address))
}

pub fn map_utxos(utxos: Vec<BitcoinUTXO>, address: String) -> Vec<UTXO> {
    utxos
        .into_iter()
        .map(|utxo| UTXO {
            transaction_id: utxo.txid,
            vout: utxo.vout,
            value: utxo.value,
            address: address.clone(),
        })
        .collect()
}

pub fn map_transaction_preload_zcash(node_info: BitcoinNodeInfo, utxos: Vec<BitcoinUTXO>, input: TransactionPreloadInput) -> Result<TransactionLoadMetadata, Box<dyn Error + Sync + Send>> {
    let output_minimum = preload_output_minimum(BitcoinChain::Zcash, &input)?;
    let utxos = map_utxos(utxos, input.sender_address);
    let branch_id = node_info.backend.consensus.ok_or("Branch ID not found")?.chaintip;
    Ok(TransactionLoadMetadata::Zcash { utxos, branch_id, output_minimum })
}
