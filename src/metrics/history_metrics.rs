const MAX_METRICS_SIZE: usize = 120;

#[derive(Default)]
pub struct HistoryMetrics {
    data: Vec<usize>,
}

impl HistoryMetrics {
    pub fn add(&mut self, value: usize) {
        if self.data.len() == MAX_METRICS_SIZE {
            self.data.remove(0);
        }

        self.data.push(value);
    }

    pub fn get_metrics(&self) -> Vec<usize> {
        self.data.clone()
    }
}
