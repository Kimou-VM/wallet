use std::cmp::Reverse;
use std::collections::BTreeSet;
use std::error::Error;

use etherscan::client::EtherscanClient;
use gem_client::Client;
use primitives::EVMChain;

use super::TransactionReference;

pub(crate) async fn get_transactions_by_address<C: Client>(client: &EtherscanClient<C>, chain: EVMChain, address: &str, limit: usize) -> Result<Vec<TransactionReference>, Box<dyn Error + Send + Sync>> {
    let (transactions, token_transfers) = futures::try_join!(client.get_transactions(chain, address, limit), client.get_token_transfers(chain, address, limit))?;
    Ok(transactions
        .into_iter()
        .chain(token_transfers)
        .map(|transaction| (Reverse(transaction.block_number), transaction.hash))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|(Reverse(block_number), hash)| TransactionReference::new(hash, Some(block_number)))
        .collect())
}

#[cfg(test)]
mod tests {
    use etherscan::testkit::{NO_TRANSACTIONS, TOKEN_TRANSFERS, TRANSACTIONS};
    use gem_client::testkit::MockClient;

    use super::*;

    #[tokio::test]
    async fn test_get_transactions_by_address() {
        let client = EtherscanClient::new_with_client(
            MockClient::new().with_get(|path| {
                let response = match path {
                    "/api?chainid=81457&module=account&action=txlist&address=0x123&page=1&offset=3&sort=desc" => TRANSACTIONS,
                    "/api?chainid=81457&module=account&action=tokentx&address=0x123&page=1&offset=3&sort=desc" => TOKEN_TRANSFERS,
                    "/api?chainid=81457&module=account&action=txlist&address=0x456&page=1&offset=3&sort=desc" => NO_TRANSACTIONS,
                    "/api?chainid=81457&module=account&action=tokentx&address=0x456&page=1&offset=3&sort=desc" => NO_TRANSACTIONS,
                    _ => panic!("unexpected path: {path}"),
                };
                Ok(response.as_bytes().to_vec())
            }),
            None,
        );

        assert_eq!(
            get_transactions_by_address(&client, EVMChain::Blast, "0x123", 3).await.unwrap(),
            vec![
                TransactionReference::new("0xtoken".to_string(), Some(11)),
                TransactionReference::new("0xnormal".to_string(), Some(10)),
                TransactionReference::new("0xshared".to_string(), Some(8))
            ]
        );
        assert_eq!(get_transactions_by_address(&client, EVMChain::Blast, "0x456", 3).await.unwrap(), vec![]);
    }
}
