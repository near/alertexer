use borsh::BorshDeserialize;
use futures::{
    stream::{self, StreamExt},
    future::try_join_all
};

use alert_rules::{AlertRule, MatchingRule};

use crate::matchers::Matcher;

pub(crate) async fn check_outcomes(context: &crate::AlertexerContext<'_>) -> anyhow::Result<()> {
    let alert_rules_inmemory_lock = context.alert_rules_inmemory.lock().await;
    // TODO: avoid cloning
    let alert_rules: Vec<alert_rules::AlertRule> =
        alert_rules_inmemory_lock.values().cloned().collect();
    drop(alert_rules_inmemory_lock);

    let execution_outcomes_rule_handler_future = alert_rules
        .iter()
        .map(|alert_rule| rule_handler(alert_rule, context));

    try_join_all(execution_outcomes_rule_handler_future).await?;

    Ok(())
}

async fn rule_handler(
    alert_rule: &AlertRule,
    context: &crate::AlertexerContext<'_>,
) -> anyhow::Result<()> {
    stream::iter(
        context
            .streamer_message
            .shards
            .iter()
            .flat_map(|shard| shard.receipt_execution_outcomes.clone())
        )
        .filter_map(|receipt_execution_outcome| async move {
            if alert_rule.matches(&receipt_execution_outcome) {
                Some(triggered_rule_handler(
                    alert_rule,
                    receipt_execution_outcome.receipt.receipt_id.to_string(),
                    context,
                ).await)
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .await;

    Ok(())
}

async fn triggered_rule_handler(
    alert_rule: &AlertRule,
    receipt_id: String,
    context: &crate::AlertexerContext<'_>,
) -> anyhow::Result<()> {
    if let Some(cache_value_bytes) =
        storage::get::<Option<Vec<u8>>>(context.redis_connection_manager, &receipt_id).await?
    {
        let cache_value = crate::cache::CacheValue::try_from_slice(&cache_value_bytes)?;

        send_trigger_to_queue(
            alert_rule,
            &cache_value.transaction_hash,
            &receipt_id,
            context,
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
    alert_rule: &AlertRule,
    transaction_hash: &str,
    receipt_id: &str,
    context: &crate::AlertexerContext<'_>,
) -> anyhow::Result<()> {
    loop {
        match shared::send_to_the_queue(
            context.queue_client,
            context.queue_url.to_string(),
            shared::types::primitives::AlertQueueMessage {
                chain_id: context.chain_id.clone(),
                alert_rule_id: alert_rule.id,
                payload: build_alert_queue_message_payload(
                    alert_rule,
                    &context.streamer_message.block.header.hash.to_string(),
                    transaction_hash,
                    receipt_id,
                ),
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

fn build_alert_queue_message_payload(
    alert_rule: &AlertRule,
    block_hash: &str,
    transaction_hash: &str,
    receipt_id: &str,
) -> shared::types::primitives::AlertQueueMessagePayload {
    match alert_rule.matching_rule() {
        MatchingRule::ActionAny { .. }
        | MatchingRule::ActionTransfer { .. }
        | MatchingRule::ActionFunctionCall { .. } => {
            shared::types::primitives::AlertQueueMessagePayload::Actions {
                block_hash: block_hash.to_string(),
                receipt_id: receipt_id.to_string(),
                transaction_hash: transaction_hash.to_string(),
            }
        }
        MatchingRule::Event { .. } => {
            shared::types::primitives::AlertQueueMessagePayload::Events {
                block_hash: block_hash.to_string(),
                receipt_id: receipt_id.to_string(),
                transaction_hash: transaction_hash.to_string(),
            }
        }
        MatchingRule::StateChangeAccountBalance { .. } => unreachable!("Unreachable code! Got StateChanges based MatchingRule we don't expect in `outcomes` checker"),
    }
}
