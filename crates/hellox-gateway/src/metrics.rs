use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default)]
pub struct GatewayMetrics {
    requests_total: AtomicU64,
    requests_failed_total: AtomicU64,
    request_latency_ms_sum: AtomicU64,
    request_latency_ms_max: AtomicU64,
}

impl GatewayMetrics {
    pub fn observe_request(&self, duration: std::time::Duration, ok: bool) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
        if !ok {
            self.requests_failed_total.fetch_add(1, Ordering::Relaxed);
        }

        let duration_ms = duration.as_millis().min(u128::from(u64::MAX)) as u64;
        self.request_latency_ms_sum
            .fetch_add(duration_ms, Ordering::Relaxed);

        let mut current = self.request_latency_ms_max.load(Ordering::Relaxed);
        while duration_ms > current {
            match self.request_latency_ms_max.compare_exchange(
                current,
                duration_ms,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(next) => current = next,
            }
        }
    }

    pub fn render_prometheus(&self) -> String {
        let requests_total = self.requests_total.load(Ordering::Relaxed);
        let requests_failed_total = self.requests_failed_total.load(Ordering::Relaxed);
        let latency_sum = self.request_latency_ms_sum.load(Ordering::Relaxed);
        let latency_max = self.request_latency_ms_max.load(Ordering::Relaxed);

        let mut lines = Vec::new();
        lines.push(
            "# HELP hellox_gateway_up Whether the gateway is running (1) or not (0).".to_string(),
        );
        lines.push("# TYPE hellox_gateway_up gauge".to_string());
        lines.push("hellox_gateway_up 1".to_string());
        lines.push(
            "# HELP hellox_gateway_requests_total Total number of /v1/messages requests processed by the gateway.".to_string(),
        );
        lines.push("# TYPE hellox_gateway_requests_total counter".to_string());
        lines.push(format!("hellox_gateway_requests_total {requests_total}"));
        lines.push(
            "# HELP hellox_gateway_requests_failed_total Total number of /v1/messages requests that returned an error status.".to_string(),
        );
        lines.push("# TYPE hellox_gateway_requests_failed_total counter".to_string());
        lines.push(format!(
            "hellox_gateway_requests_failed_total {requests_failed_total}"
        ));
        lines.push(
            "# HELP hellox_gateway_request_latency_ms_sum Sum of request latencies (ms) for /v1/messages.".to_string(),
        );
        lines.push("# TYPE hellox_gateway_request_latency_ms_sum counter".to_string());
        lines.push(format!(
            "hellox_gateway_request_latency_ms_sum {latency_sum}"
        ));
        lines.push(
            "# HELP hellox_gateway_request_latency_ms_max Max request latency (ms) observed for /v1/messages.".to_string(),
        );
        lines.push("# TYPE hellox_gateway_request_latency_ms_max gauge".to_string());
        lines.push(format!(
            "hellox_gateway_request_latency_ms_max {latency_max}"
        ));

        lines.join("\n") + "\n"
    }
}
