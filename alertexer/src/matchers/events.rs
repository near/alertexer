use near_lake_framework::near_indexer_primitives::IndexerExecutionOutcomeWithReceipt;

use shared::types::events::Event;

pub(crate) fn match_events(
    account_id: &str,
    event: &str,
    standard: &str,
    version: &str,
    outcome_with_receipt: &IndexerExecutionOutcomeWithReceipt,
) -> bool {
    if super::match_account(account_id, outcome_with_receipt) {
        let outcome_logs_with_triggered_events_json: Vec<Event> = outcome_with_receipt
            .execution_outcome
            .outcome
            .logs
            .iter()
            .filter_map(|log| Event::from_log(log).ok())
            .filter(|near_event| {
                vec![
                    wildmatch::WildMatch::new(event).matches(&near_event.event),
                    wildmatch::WildMatch::new(standard).matches(&near_event.standard),
                    wildmatch::WildMatch::new(version).matches(&near_event.version),
                ]
                .into_iter()
                .all(|val| val)
            })
            .collect();
        if !outcome_logs_with_triggered_events_json.is_empty() {
            return true;
        }
    }
    false
}
