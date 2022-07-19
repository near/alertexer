use borsh::BorshDeserialize;
use futures::future::try_join_all;

use alert_rules::AlertRule;

use near_lake_framework::near_indexer_primitives::IndexerExecutionOutcomeWithReceipt;

use crate::matchers::Matcher;

pub(crate) async fn check_outcomes(
    streamer_message: &near_lake_framework::near_indexer_primitives::StreamerMessage,
    chain_id: &shared::types::primitives::ChainId,
    alert_rules: &[AlertRule],
    redis_connection_manager: &storage::ConnectionManager,
    queue_client: &shared::QueueClient,
    queue_url: &str,
) -> anyhow::Result<()> {
    let block_hash_string = streamer_message.block.header.hash.to_string();

    let receipt_execution_outcomes: Vec<IndexerExecutionOutcomeWithReceipt> = streamer_message
        .shards
        .iter()
        .flat_map(|shard| shard.receipt_execution_outcomes.clone())
        .collect();

    let execution_outcomes_rule_handler_future = alert_rules.iter().map(|alert_rule| {
        rule_handler(
            &block_hash_string,
            chain_id,
            alert_rule,
            &receipt_execution_outcomes,
            redis_connection_manager,
            queue_client,
            queue_url,
        )
    });

    try_join_all(execution_outcomes_rule_handler_future).await?;

    Ok(())
}

async fn rule_handler(
    block_hash: &str,
    chain_id: &shared::types::primitives::ChainId,
    alert_rule: &AlertRule,
    receipt_execution_outcomes: &[IndexerExecutionOutcomeWithReceipt],
    redis_connection_manager: &storage::ConnectionManager,
    queue_client: &shared::QueueClient,
    queue_url: &str,
) -> anyhow::Result<()> {
    let triggered_rules_futures = receipt_execution_outcomes
        .iter()
        .filter(|receipt_execution_outcome| alert_rule.matches(receipt_execution_outcome))
        .map(|receipt_execution_outcome| {
            triggered_rule_handler(
                block_hash,
                chain_id,
                alert_rule,
                receipt_execution_outcome,
                redis_connection_manager,
                queue_client,
                queue_url,
            )
        });

    try_join_all(triggered_rules_futures).await?;

    Ok(())
}

async fn triggered_rule_handler(
    block_hash: &str,
    chain_id: &shared::types::primitives::ChainId,
    alert_rule: &AlertRule,
    receipt_execution_outcome: &IndexerExecutionOutcomeWithReceipt,
    redis_connection_manager: &storage::ConnectionManager,
    queue_client: &shared::QueueClient,
    queue_url: &str,
) -> anyhow::Result<()> {
    let receipt_id = receipt_execution_outcome.receipt.receipt_id.to_string();
    if let Some(cache_value_bytes) =
        storage::get::<Option<Vec<u8>>>(redis_connection_manager, &receipt_id).await?
    {
        let cache_value = crate::cache::CacheValue::try_from_slice(&cache_value_bytes)?;

        send_trigger_to_queue(
            block_hash,
            chain_id,
            alert_rule,
            &cache_value.transaction_hash,
            &receipt_id,
            queue_client,
            queue_url,
        )
        .await?;
    } else {
        tracing::error!(
            target: crate::INDEXER,
            "Missing Receipt {}. Not found in watching list",
            &receipt_id,
        );
    }
    Ok(())
}

async fn send_trigger_to_queue(
    block_hash: &str,
    chain_id: &shared::types::primitives::ChainId,
    alert_rule: &AlertRule,
    transaction_hash: &str,
    receipt_id: &str,
    queue_client: &shared::QueueClient,
    queue_url: &str,
) -> anyhow::Result<()> {
    loop {
        match shared::send_to_the_queue(
            queue_client,
            queue_url.to_string(),
            shared::types::primitives::AlertQueueMessage {
                chain_id: chain_id.clone(),
                alert_rule_id: alert_rule.id,
                payload: shared::types::primitives::AlertQueueMessagePayload::Events {
                    block_hash: block_hash.to_string(),
                    receipt_id: receipt_id.to_string(),
                    transaction_hash: transaction_hash.to_string(),
                },
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
