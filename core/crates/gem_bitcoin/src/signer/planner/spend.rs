use bitcoin::{Amount, ScriptBuf};
use primitives::{BitcoinChain, SignerError};

use super::{
    PlanInput, PlanOutput, SpendPlan, SpendRequest,
    fee::estimate_fee,
    inputs::spendable_inputs,
    outputs::{dust_threshold, op_return_output, spend_outputs},
};
use crate::signer::address::script_for_address;

pub(crate) struct UtxoPlanner;

#[derive(Debug, Clone, Copy)]
enum SpendTarget {
    Exact(u64),
    Max,
}

struct Spend {
    chain: BitcoinChain,
    target: SpendTarget,
    fee_rate: u64,
    payment_script: ScriptBuf,
    change_script: ScriptBuf,
    memo_output: Option<PlanOutput>,
    inputs: Vec<PlanInput>,
    dust_change_to_fee: bool,
}

impl UtxoPlanner {
    pub(crate) fn plan(request: SpendRequest) -> Result<SpendPlan, SignerError> {
        Spend::new(request)?.plan().ok_or(SignerError::InsufficientFunds)
    }

    pub(crate) fn fee(request: SpendRequest) -> Result<u64, SignerError> {
        let spend = Spend::new(request)?;
        Ok(match spend.plan() {
            Some(plan) => plan.fee,
            None => estimate_fee(spend.chain, &spend.inputs, &spend.outputs(), spend.fee_rate),
        })
    }
}

impl Spend {
    fn new(request: SpendRequest) -> Result<Self, SignerError> {
        if request.utxos.is_empty() {
            return SignerError::invalid_input_err("missing input UTXOs");
        }

        let payment_script = script_for_address(request.chain, &request.destination_address)?.script_pubkey;
        let change_script = script_for_address(request.chain, &request.sender_address)?.script_pubkey;
        let memo_output = request.memo.as_deref().map(op_return_output).transpose()?;
        let mut inputs = spendable_inputs(request.chain, &request.sender_address, request.utxos)?;
        inputs.sort_by(|left, right| left.value.to_sat().cmp(&right.value.to_sat()).then_with(|| left.previous_output.cmp(&right.previous_output)));
        if inputs.iter().map(|input| input.value.to_sat() as u128).sum::<u128>() > u64::MAX as u128 {
            return SignerError::invalid_input_err("Bitcoin amount overflow");
        }

        Ok(Self {
            chain: request.chain,
            target: if request.is_max { SpendTarget::Max } else { SpendTarget::Exact(request.amount) },
            fee_rate: request.fee_rate,
            payment_script,
            change_script,
            memo_output,
            inputs,
            dust_change_to_fee: request.dust_change_to_fee,
        })
    }

    fn plan(&self) -> Option<SpendPlan> {
        let selections = || (1..=self.inputs.len()).map(|count| &self.inputs[..count]);
        match self.target {
            SpendTarget::Max => self.plan_with(&self.inputs),
            SpendTarget::Exact(value) => selections()
                .find_map(|inputs| self.plan_with(inputs))
                .or_else(|| selections().filter(|_| self.dust_change_to_fee).find_map(|inputs| self.plan_with_change_to_fee(value, inputs))),
        }
    }

    fn plan_with(&self, inputs: &[PlanInput]) -> Option<SpendPlan> {
        let mut outputs = self.outputs();
        let fee = estimate_fee(self.chain, inputs, &outputs, self.fee_rate);
        let spendable = inputs.iter().map(|input| input.value.to_sat()).sum::<u64>().checked_sub(fee)?;
        let (output, value) = match self.target {
            SpendTarget::Max => (outputs.first_mut()?, spendable),
            SpendTarget::Exact(value) => (outputs.last_mut()?, spendable.checked_sub(value).filter(|change| *change >= dust_threshold(self.chain, &self.change_script))?),
        };
        output.value = Amount::from_sat(value);
        Some(SpendPlan { inputs: inputs.to_vec(), outputs, fee })
    }

    fn plan_with_change_to_fee(&self, value: u64, inputs: &[PlanInput]) -> Option<SpendPlan> {
        let outputs = spend_outputs(value, self.payment_script.clone(), self.memo_output.clone());
        let minimum_fee = estimate_fee(self.chain, inputs, &outputs, self.fee_rate);
        let fee = inputs.iter().map(|input| input.value.to_sat()).sum::<u64>().checked_sub(value).filter(|fee| *fee >= minimum_fee)?;
        Some(SpendPlan { inputs: inputs.to_vec(), outputs, fee })
    }

    fn outputs(&self) -> Vec<PlanOutput> {
        match self.target {
            SpendTarget::Max => spend_outputs(0, self.payment_script.clone(), self.memo_output.clone()),
            SpendTarget::Exact(value) => {
                let mut outputs = spend_outputs(value, self.payment_script.clone(), self.memo_output.clone());
                outputs.push(PlanOutput::new(0, self.change_script.clone()));
                outputs
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{
        address_mock::{TEST_BITCOIN_P2WPKH_ADDRESS, mock_destination_address, mock_sender_address},
        planner_mock::{TEST_SPEND_RECIPIENT, mock_signer_input, mock_signer_input_with, mock_spend_utxos, sum_inputs},
        signer_mock::{TEST_UTXO_TXID, mock_transfer_input_with_utxos, mock_transfer_swap_input, mock_utxo_with},
    };
    use num_bigint::BigInt;
    use primitives::{AssetId, Chain, GasPriceType, SignerInput, TransactionFee, TransactionInputType, TransactionLoadMetadata, TransferAmount, TransferAmountError, TransferAmountInput};

    fn plan_with_fee_rate(value: &str, fee_rate: u64) -> SpendPlan {
        let mut input = mock_signer_input(value, false);
        input.input.gas_price = GasPriceType::regular(BigInt::from(fee_rate));
        input.fee = TransactionFee::new_from_fee(BigInt::from(fee_rate), AssetId::from_chain(Chain::Bitcoin));
        UtxoPlanner::plan(SpendRequest::transfer(BitcoinChain::Bitcoin, &input).unwrap()).unwrap()
    }

    #[test]
    fn test_plan_applies_fractional_fee_rate_below_one_sat() {
        let sub_one = plan_with_fee_rate("5000", 1);
        let one_sat = plan_with_fee_rate("5000", 10);

        assert!(sub_one.fee > 0);
        assert!(sub_one.fee < one_sat.fee);
        assert_eq!(sub_one.outputs[0].value.to_sat(), 5_000);
    }

    fn shown_amount(chain: BitcoinChain, input: &SignerInput, available: u64) -> Result<TransferAmount, TransferAmountError> {
        TransferAmountInput {
            fee: BigInt::from(UtxoPlanner::fee(SpendRequest::transfer(chain, input).unwrap()).unwrap()),
            is_max_amount: input.is_max_value,
            output_minimum: input.metadata.get_output_minimum(),
            ..TransferAmountInput::mock(input.input_type.clone(), input.value_as_u64().unwrap(), available, available)
        }
        .calculate()
    }

    #[test]
    fn test_plan_transfer() {
        let request = SpendRequest::transfer(BitcoinChain::Bitcoin, &mock_signer_input("12000", false)).unwrap();
        let plan = UtxoPlanner::plan(request).unwrap();
        assert_eq!(plan.inputs.len(), 2);
        assert_eq!(plan.outputs.len(), 3);
        assert_eq!(plan.outputs[0].value.to_sat(), 12_000);
        assert_eq!(plan.outputs[1].value.to_sat(), 0);
        assert!(plan.outputs[1].script_pubkey.is_op_return());
        assert_eq!(sum_inputs(&plan.inputs).unwrap(), plan.outputs.iter().map(|output| output.value.to_sat()).sum::<u64>() + plan.fee);
        assert_eq!(plan.fee, 454);
        assert_eq!(
            shown_amount(BitcoinChain::Bitcoin, &mock_signer_input("12000", false), 30_000).unwrap(),
            TransferAmount {
                value: BigInt::from(12_000),
                network_fee: BigInt::from(454),
                is_max_amount: false,
            }
        );

        let request = SpendRequest::transfer(BitcoinChain::Bitcoin, &mock_signer_input("9600", false)).unwrap();
        let plan = UtxoPlanner::plan(request).unwrap();
        assert_eq!(plan.inputs.len(), 2, "the first input alone would leave dust change, so another input keeps a valid change");
        assert_eq!(plan.outputs[0].value.to_sat(), 9_600);
        assert_eq!(plan.outputs[2].value.to_sat(), 30_000 - 9_600 - plan.fee);

        let request = SpendRequest::transfer(BitcoinChain::Bitcoin, &mock_signer_input("50000", false)).unwrap();
        assert_eq!(UtxoPlanner::plan(request.clone()).unwrap_err(), SignerError::InsufficientFunds);
        assert_eq!(UtxoPlanner::fee(request).unwrap(), 454, "the fee load prices the send it was asked for instead of refusing it");

        let dust_max_utxos = vec![mock_utxo_with(TEST_UTXO_TXID, 0, "600", TEST_BITCOIN_P2WPKH_ADDRESS)];
        let dust_max = mock_signer_input_with("600", true, None, dust_max_utxos);
        assert_eq!(
            shown_amount(BitcoinChain::Bitcoin, &dust_max, 600),
            Err(TransferAmountError::DustThreshold {
                asset_id: AssetId::from_chain(Chain::Bitcoin),
                minimum: BigInt::from(546),
                value: BigInt::from(374),
            })
        );

        let request = SpendRequest::transfer(BitcoinChain::Bitcoin, &mock_signer_input_with("1000", false, Some("a".repeat(81)), mock_spend_utxos())).unwrap();
        assert_eq!(UtxoPlanner::plan(request).unwrap_err(), SignerError::invalid_input("Bitcoin memo is too large"));

        let request = SpendRequest::transfer(BitcoinChain::Bitcoin, &mock_signer_input("0", true)).unwrap();
        let plan = UtxoPlanner::plan(request).unwrap();
        assert_eq!(plan.inputs.len(), 2);
        assert_eq!(plan.outputs.len(), 2);
        assert!(plan.outputs[1].script_pubkey.is_op_return());
        assert_eq!(sum_inputs(&plan.inputs).unwrap(), plan.outputs.iter().map(|output| output.value.to_sat()).sum::<u64>() + plan.fee);
    }

    #[test]
    fn test_amount_under_dust() {
        assert_eq!(
            shown_amount(BitcoinChain::Bitcoin, &mock_signer_input("545", false), 30_000),
            Err(TransferAmountError::DustThreshold {
                asset_id: AssetId::from_chain(Chain::Bitcoin),
                minimum: BigInt::from(546),
                value: BigInt::from(545),
            })
        );
        assert!(shown_amount(BitcoinChain::Bitcoin, &mock_signer_input("546", false), 30_000).is_ok());
    }

    #[test]
    fn test_near_empty_send_is_signed_as_shown() {
        let doge_sender = mock_sender_address(BitcoinChain::Doge);
        let doge = mock_transfer_input_with_utxos(
            BitcoinChain::Doge,
            &doge_sender,
            &mock_destination_address(BitcoinChain::Doge),
            "1500000",
            vec![mock_utxo_with(TEST_UTXO_TXID, 0, "2000000", &doge_sender)],
        );
        let cases = [
            (BitcoinChain::Bitcoin, mock_signer_input("29446", false), 30_000),
            (BitcoinChain::Bitcoin, mock_signer_input("29300", false), 30_000),
            (BitcoinChain::Bitcoin, mock_signer_input("29900", false), 30_000),
            (BitcoinChain::Doge, doge, 2_000_000),
        ];
        for (chain, input, balance) in cases {
            let value = input.value.clone();
            assert_eq!(UtxoPlanner::plan(SpendRequest::transfer(chain, &input).unwrap()).unwrap_err(), SignerError::InsufficientFunds, "{chain:?} {value}");
            assert!(
                shown_amount(chain, &input, balance).unwrap().is_max_amount,
                "{chain:?} {value}: what is left is under the change dust, so the send becomes a send of everything"
            );

            let mut send_everything = input;
            send_everything.input.value = balance.into();
            send_everything.input.is_max_value = true;
            let shown = shown_amount(chain, &send_everything, balance).unwrap();
            let signed = UtxoPlanner::plan(SpendRequest::transfer(chain, &send_everything).unwrap()).unwrap();
            assert_eq!((shown.value, shown.network_fee), (BigInt::from(signed.outputs[0].value.to_sat()), BigInt::from(signed.fee)), "{chain:?} {value}");
            assert_eq!(signed.outputs.iter().map(|output| output.value.to_sat()).sum::<u64>() + signed.fee, balance, "{chain:?} {value}: no change is left");
        }
    }

    fn swap_input(value: &str) -> SignerInput {
        let mut input = mock_signer_input(value, false);
        input.input.input_type = mock_transfer_swap_input(BitcoinChain::Bitcoin, "memo").input.input_type;
        if let TransactionInputType::Swap { swap_data, .. } = &mut input.input.input_type {
            swap_data.data.to = TEST_SPEND_RECIPIENT.to_string();
            swap_data.data.value = value.parse().unwrap();
        }
        if let TransactionLoadMetadata::Bitcoin { output_minimum, .. } = &mut input.input.metadata {
            output_minimum.change = 0;
        }
        input
    }

    #[test]
    fn test_swap_dust_change_goes_to_the_fee() {
        let input = swap_input("29300");
        let signed = UtxoPlanner::plan(SpendRequest::swap(BitcoinChain::Bitcoin, &input).unwrap()).unwrap();
        assert_eq!(signed.outputs.len(), 2, "no change output");
        assert_eq!(signed.outputs[0].value.to_sat(), 29_300);
        assert_eq!(signed.fee, 700, "the 246 sats of dust change go to the fee");

        let shown = TransferAmountInput {
            fee: BigInt::from(UtxoPlanner::fee(SpendRequest::swap(BitcoinChain::Bitcoin, &input).unwrap()).unwrap()),
            output_minimum: input.metadata.get_output_minimum(),
            ..TransferAmountInput::mock(input.input_type.clone(), 29_300, 30_000, 30_000)
        }
        .calculate()
        .unwrap();
        assert_eq!(
            shown,
            TransferAmount {
                value: BigInt::from(29_300),
                network_fee: BigInt::from(signed.fee),
                is_max_amount: false,
            }
        );

        let send = SpendRequest::transfer(BitcoinChain::Bitcoin, &mock_signer_input("29300", false)).unwrap();
        assert_eq!(UtxoPlanner::plan(send).unwrap_err(), SignerError::InsufficientFunds, "a send leaving dust becomes a send of everything instead");
    }

    #[test]
    fn test_dust_threshold_is_script_aware() {
        let p2wpkh = script_for_address(BitcoinChain::Bitcoin, TEST_BITCOIN_P2WPKH_ADDRESS).unwrap().script_pubkey;
        let p2pkh = script_for_address(BitcoinChain::Bitcoin, "1BoatSLRHtKNngkdXEeobR76b53LETtpyT").unwrap().script_pubkey;
        let doge_p2pkh = script_for_address(BitcoinChain::Doge, &mock_destination_address(BitcoinChain::Doge)).unwrap().script_pubkey;

        assert_eq!(dust_threshold(BitcoinChain::Bitcoin, &p2pkh), 546);
        assert_eq!(dust_threshold(BitcoinChain::Doge, &doge_p2pkh), 546_000);
        assert!(dust_threshold(BitcoinChain::Bitcoin, &p2wpkh) > 0 && dust_threshold(BitcoinChain::Bitcoin, &p2wpkh) < dust_threshold(BitcoinChain::Bitcoin, &p2pkh));
    }
}
