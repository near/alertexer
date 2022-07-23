use alert_rules::{AlertRule, MatchingRule, Status};
use near_lake_framework::near_indexer_primitives::{
    views::ExecutionStatusView, IndexerExecutionOutcomeWithReceipt,
};

mod actions;
mod events;
pub(crate) mod state_changes;

pub trait Matcher {
    fn matches(&self, outcome_with_receipt: &IndexerExecutionOutcomeWithReceipt) -> bool;
}

impl Matcher for AlertRule {
    fn matches(&self, outcome_with_receipt: &IndexerExecutionOutcomeWithReceipt) -> bool {
        self.matching_rule().matches(outcome_with_receipt)
    }
}

impl Matcher for MatchingRule {
    fn matches(&self, outcome_with_receipt: &IndexerExecutionOutcomeWithReceipt) -> bool {
        match self {
            MatchingRule::ActionAny {
                affected_account_id,
                status,
            } => actions::match_action_any(affected_account_id, status, outcome_with_receipt),
            &MatchingRule::ActionTransfer { .. } => {
                tracing::debug!(
                    target: crate::INDEXER,
                    "ActionTransfer matcher is not implemented"
                );
                false
            }
            MatchingRule::ActionFunctionCall {
                affected_account_id,
                status,
                function,
            } => actions::match_action_function_call(
                affected_account_id,
                status,
                function,
                outcome_with_receipt,
            ),
            MatchingRule::Event {
                contract_account_id,
                event,
                standard,
                version,
            } => events::match_events(
                contract_account_id,
                event,
                standard,
                version,
                outcome_with_receipt,
            ),
            MatchingRule::StateChangeAccountBalance { .. } => unreachable!("Unreachable code! Didn't expect StateChanges based MatchingRule in `outcomes` checker"),
        }
    }
}

fn match_account(
    account_id: &str,
    outcome_with_receipt: &IndexerExecutionOutcomeWithReceipt,
) -> bool {
    wildmatch::WildMatch::new(account_id).matches(&outcome_with_receipt.receipt.receiver_id)
        || wildmatch::WildMatch::new(account_id)
            .matches(&outcome_with_receipt.receipt.predecessor_id)
}

fn match_status(status: &Status, execution_outcome_status: &ExecutionStatusView) -> bool {
    match status {
        Status::Any => true,
        Status::Success => match execution_outcome_status {
            ExecutionStatusView::SuccessValue(_) | ExecutionStatusView::SuccessReceiptId(_) => true,
            _ => false,
        },
        Status::Fail => match execution_outcome_status {
            ExecutionStatusView::SuccessValue(_) | ExecutionStatusView::SuccessReceiptId(_) => {
                false
            }
            _ => true,
        },
    }
}
