use borsh::{BorshDeserialize, BorshSerialize};
use near_lake_framework::near_indexer_primitives::{views, IndexerTransactionWithOutcome};
use serde::{Deserialize, Serialize};

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Debug, Clone)]
pub struct TransactionDetails {
    pub transaction: views::SignedTransactionView,
    pub receipts: Vec<views::ReceiptView>,
    pub execution_outcomes: Vec<views::ExecutionOutcomeWithIdView>,
}

impl TransactionDetails {
    pub fn from_indexer_tx(transaction: IndexerTransactionWithOutcome) -> Self {
        Self {
            transaction: transaction.transaction.clone(),
            receipts: vec![],
            execution_outcomes: vec![transaction.outcome.execution_outcome],
        }
    }
}
