use crate::metrics::HistoryMetrics;

#[derive(Default)]
pub struct GatewayConnectionMetrics {
    pub in_per_second: HistoryMetrics,
    pub out_per_second: HistoryMetrics,
}
