use borsh::BorshDeserialize;
use futures::future::try_join_all;

use alert_rules::{AlertRule, MatchingRule};

use near_lake_framework::near_indexer_primitives::views::{
    StateChangeCauseView, StateChangeWithCauseView,
};
use shared::types::primitives::{AlertQueueMessage, AlertQueueMessagePayload};

pub(crate) async fn check_state_changes(
    state_changes: &[StateChangeWithCauseView],
    current_block_hash: &str,
    prev_block_hash: &str,
    chain_id: &shared::types::primitives::ChainId,
    alert_rules: &[AlertRule],
    balance_cache: &crate::BalanceCache,
    redis_connection_manager: &storage::ConnectionManager,
    queue_client: &shared::QueueClient,
    queue_url: &str,
    json_rpc_client: &near_jsonrpc_client::JsonRpcClient,
) -> anyhow::Result<()> {
    let state_changes_rule_handler_future = alert_rules.iter().map(|alert_rule| {
        rule_handler(
            state_changes,
            current_block_hash,
            prev_block_hash,
            chain_id,
            alert_rule,
            balance_cache,
            redis_connection_manager,
            queue_client,
            queue_url,
            json_rpc_client,
        )
    });

    try_join_all(state_changes_rule_handler_future).await?;

    Ok(())
}

async fn rule_handler(
    state_changes: &[StateChangeWithCauseView],
    current_block_hash: &str,
    prev_block_hash: &str,
    chain_id: &shared::types::primitives::ChainId,
    alert_rule: &AlertRule,
    balance_cache: &crate::BalanceCache,
    redis_connection_manager: &storage::ConnectionManager,
    queue_client: &shared::QueueClient,
    queue_url: &str,
    json_rpc_client: &near_jsonrpc_client::JsonRpcClient,
) -> anyhow::Result<()> {
    let mut triggered_rules_futures = vec![];
    for state_change_with_cause in state_changes {
        match alert_rule.matching_rule() {
            MatchingRule::StateChangeAccountBalance {
                affected_account_id,
                comparator,
            } => {
                if crate::matchers::state_changes::match_state_change_account_balance(
                    affected_account_id,
                    comparator,
                    state_change_with_cause,
                    prev_block_hash,
                    balance_cache,
                    json_rpc_client,
                )
                .await
                {
                    triggered_rules_futures.push(triggered_rule_handler(
                        state_change_with_cause,
                        current_block_hash,
                        chain_id,
                        alert_rule,
                        redis_connection_manager,
                        queue_client,
                        queue_url,
                    ));
                }
            }
            _ => unreachable!("Unreachable code! Didn't expect to get ExecutionOutcome or Receipt based MatchingRule in `state_changes` checker"),
        }
    }

    try_join_all(triggered_rules_futures).await?;

    Ok(())
}

async fn triggered_rule_handler(
    state_change_with_cause: &StateChangeWithCauseView,
    block_hash: &str,
    chain_id: &shared::types::primitives::ChainId,
    alert_rule: &AlertRule,
    redis_connection_manager: &storage::ConnectionManager,
    queue_client: &shared::QueueClient,
    queue_url: &str,
) -> anyhow::Result<()> {
    let payload = match state_change_with_cause.cause {
        StateChangeCauseView::TransactionProcessing { tx_hash } => {
            AlertQueueMessagePayload::StateChanges {
                block_hash: block_hash.to_string(),
                receipt_id: None,
                transaction_hash: tx_hash.to_string(),
            }
        }
        StateChangeCauseView::ActionReceiptProcessingStarted { receipt_hash }
        | StateChangeCauseView::ActionReceiptGasReward { receipt_hash }
        | StateChangeCauseView::ReceiptProcessing { receipt_hash }
        | StateChangeCauseView::PostponedReceipt { receipt_hash } => {
            let transaction_hash_string = get_parent_tx_for_receipt_from_cache(
                &receipt_hash.to_string(),
                redis_connection_manager,
            )
            .await?
            .expect("Failed to get parent transaction hash from the cache");
            AlertQueueMessagePayload::StateChanges {
                block_hash: block_hash.to_string(),
                receipt_id: Some(receipt_hash.to_string()),
                transaction_hash: transaction_hash_string,
            }
        }
        _ => {
            unreachable!("Unreachable code. Didn't expect to process StateChangeCause that doesn't include either transation_hash or receipt_hash");
        }
    };

    loop {
        match shared::send_to_the_queue(
            queue_client,
            queue_url.to_string(),
            AlertQueueMessage {
                chain_id: chain_id.clone(),
                alert_rule_id: alert_rule.id,
                payload: payload.clone(),
            },
        )
        .await
        {
            Ok(_) => break,
            Err(err) => {
                tracing::error!(
                    target: crate::INDEXER,
                    "Error sending the alert to the queue. Retrying in 1s...\n{:#?}",
                    err,
                );
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
    }

    Ok(())
}

async fn get_parent_tx_for_receipt_from_cache(
    receipt_id: &str,
    redis_connection_manager: &storage::ConnectionManager,
) -> anyhow::Result<Option<String>> {
    if let Some(cache_value_bytes) =
        storage::get::<Option<Vec<u8>>>(redis_connection_manager, receipt_id).await?
    {
        let cache_value = crate::cache::CacheValue::try_from_slice(&cache_value_bytes)?;

        Ok(Some(cache_value.transaction_hash))
    } else {
        Ok(None)
    }
}
