use crate::cache::monitoring::CacheMonitor;
use crate::cache::monitoring::CacheStatistics;
use crate::cache::reliability::{SloViolation, ViolationSeverity};
// use axum::{
//     extract::{Query, State},
//     http::StatusCode,
//     response::{Html, Json},
//     routing::{get, post},
//     Router,
// };
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock;

/// Cache analytics dashboard for real-time monitoring and insights
pub struct AnalyticsDashboard {
    metrics: Arc<CacheMonitor>,
    time_series_data: Arc<RwLock<TimeSeriesData>>,
    config: DashboardConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardConfig {
    /// Port to serve dashboard on
    pub port: u16,
    /// Data retention period
    pub retention_period: Duration,
    /// Update interval
    pub update_interval: Duration,
    /// Enable real-time updates
    pub enable_realtime: bool,
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            port: 9091,
            retention_period: Duration::from_secs(86400), // 24 hours
            update_interval: Duration::from_secs(5),
            enable_realtime: true,
        }
    }
}

struct TimeSeriesData {
    hit_rate_history: VecDeque<DataPoint<f64>>,
    latency_history: VecDeque<DataPoint<f64>>,
    throughput_history: VecDeque<DataPoint<f64>>,
    error_rate_history: VecDeque<DataPoint<f64>>,
    cache_size_history: VecDeque<DataPoint<u64>>,
    eviction_history: VecDeque<DataPoint<u64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DataPoint<T> {
    timestamp: SystemTime,
    value: T,
}

impl AnalyticsDashboard {
    pub fn new(metrics: Arc<CacheMonitor>, config: DashboardConfig) -> Self {
        Self {
            metrics,
            time_series_data: Arc::new(RwLock::new(TimeSeriesData {
                hit_rate_history: VecDeque::new(),
                latency_history: VecDeque::new(),
                throughput_history: VecDeque::new(),
                error_rate_history: VecDeque::new(),
                cache_size_history: VecDeque::new(),
                eviction_history: VecDeque::new(),
            })),
            config,
        }
    }

    /// Start the analytics dashboard server
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Start background data collection
        let collector = self.spawn_data_collector();

        // Create web server
        let app = self.create_app().await;

        // Start server
        let addr = format!("0.0.0.0:{}", self.config.port);
        axum::Server::bind(&addr.parse()?)
            .serve(app.into_make_service())
            .await?;

        Ok(())
    }

    fn spawn_data_collector(&self) -> tokio::task::JoinHandle<()> {
        let metrics = Arc::clone(&self.metrics);
        let time_series = Arc::clone(&self.time_series_data);
        let interval = self.config.update_interval;
        let retention = self.config.retention_period;

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;

                // Collect current metrics
                let stats = Self::collect_current_stats(&metrics).await;

                // Update time series data
                let mut data = time_series.write().await;
                let now = SystemTime::now();

                // Add new data points
                data.hit_rate_history.push_back(DataPoint {
                    timestamp: now,
                    value: stats.hit_rate,
                });
                data.latency_history.push_back(DataPoint {
                    timestamp: now,
                    value: stats.avg_latency,
                });
                data.throughput_history.push_back(DataPoint {
                    timestamp: now,
                    value: stats.throughput,
                });
                data.error_rate_history.push_back(DataPoint {
                    timestamp: now,
                    value: stats.error_rate,
                });
                data.cache_size_history.push_back(DataPoint {
                    timestamp: now,
                    value: stats.cache_size_bytes,
                });
                data.eviction_history.push_back(DataPoint {
                    timestamp: now,
                    value: stats.evictions_per_second,
                });

                // Clean up old data
                let cutoff = now - retention;
                Self::cleanup_old_data(&mut data.hit_rate_history, cutoff);
                Self::cleanup_old_data(&mut data.latency_history, cutoff);
                Self::cleanup_old_data(&mut data.throughput_history, cutoff);
                Self::cleanup_old_data(&mut data.error_rate_history, cutoff);
                Self::cleanup_old_data(&mut data.cache_size_history, cutoff);
                Self::cleanup_old_data(&mut data.eviction_history, cutoff);
            }
        })
    }

    async fn collect_current_stats(metrics: &CacheMonitor) -> CurrentStats {
        // In real implementation, would collect from metrics
        CurrentStats {
            hit_rate: 0.85,
            avg_latency: 5.2,
            throughput: 1500.0,
            error_rate: 0.001,
            cache_size_bytes: 1024 * 1024 * 100,
            evictions_per_second: 10,
        }
    }

    fn cleanup_old_data<T>(data: &mut VecDeque<DataPoint<T>>, cutoff: SystemTime) {
        while let Some(front) = data.front() {
            if front.timestamp < cutoff {
                data.pop_front();
            } else {
                break;
            }
        }
    }

    async fn create_app(&self) -> Router {
        let state = AppState {
            metrics: Arc::clone(&self.metrics),
            time_series: Arc::clone(&self.time_series_data),
        };

        Router::new()
            .route("/", get(serve_dashboard))
            .route("/api/stats", get(get_current_stats))
            .route("/api/timeseries", get(get_time_series))
            .route("/api/alerts", get(get_alerts))
            .route("/api/heatmap", get(get_heatmap))
            .route("/api/top_keys", get(get_top_keys))
            .with_state(state)
    }
}

#[derive(Clone)]
struct AppState {
    metrics: Arc<CacheMonitor>,
    time_series: Arc<RwLock<TimeSeriesData>>,
}

#[derive(Debug, Serialize)]
struct CurrentStats {
    hit_rate: f64,
    avg_latency: f64,
    throughput: f64,
    error_rate: f64,
    cache_size_bytes: u64,
    evictions_per_second: u64,
}

async fn serve_dashboard() -> Html<&'static str> {
    Html(include_str!("../../../templates/cache_dashboard.html"))
}

async fn get_current_stats(State(state): State<AppState>) -> Json<CurrentStats> {
    let stats = AnalyticsDashboard::collect_current_stats(&state.metrics).await;
    Json(stats)
}

async fn get_time_series(
    State(state): State<AppState>,
    Query(params): Query<TimeSeriesParams>,
) -> Json<TimeSeriesResponse> {
    let data = state.time_series.read().await;

    let response = TimeSeriesResponse {
        hit_rate: data.hit_rate_history.iter().cloned().collect(),
        latency: data.latency_history.iter().cloned().collect(),
        throughput: data.throughput_history.iter().cloned().collect(),
        error_rate: data.error_rate_history.iter().cloned().collect(),
        cache_size: data.cache_size_history.iter().cloned().collect(),
        evictions: data.eviction_history.iter().cloned().collect(),
    };

    Json(response)
}

async fn get_alerts() -> Json<AlertsResponse> {
    // In real implementation, would get from SLO monitor
    let alerts = vec![Alert {
        id: "1".to_string(),
        timestamp: SystemTime::now(),
        severity: AlertSeverity::Warning,
        title: "High cache miss rate".to_string(),
        description: "Cache hit rate dropped below 80%".to_string(),
        affected_metric: "hit_rate".to_string(),
    }];

    Json(AlertsResponse { alerts })
}

async fn get_heatmap() -> Json<HeatmapData> {
    // Generate sample heatmap data
    let mut data = Vec::new();
    let now = SystemTime::now();

    for hour in 0..24 {
        for day in 0..7 {
            data.push(HeatmapCell {
                hour,
                day,
                value: (hour * day) as f64 * 0.1 + 0.5,
                timestamp: now - Duration::from_secs((day * 24 + hour) * 3600),
            });
        }
    }

    Json(HeatmapData { cells: data })
}

async fn get_top_keys() -> Json<TopKeysResponse> {
    // In real implementation, would track actual key usage
    let keys = vec![
        TopKey {
            key: "task:build:main".to_string(),
            hits: 15234,
            misses: 234,
            size_bytes: 1024 * 512,
            last_access: SystemTime::now(),
        },
        TopKey {
            key: "task:test:unit".to_string(),
            hits: 12543,
            misses: 123,
            size_bytes: 1024 * 256,
            last_access: SystemTime::now() - Duration::from_secs(60),
        },
        TopKey {
            key: "task:lint:all".to_string(),
            hits: 8932,
            misses: 89,
            size_bytes: 1024 * 128,
            last_access: SystemTime::now() - Duration::from_secs(120),
        },
    ];

    Json(TopKeysResponse { keys })
}

#[derive(Debug, Deserialize)]
struct TimeSeriesParams {
    start: Option<u64>,
    end: Option<u64>,
    resolution: Option<String>,
}

#[derive(Debug, Serialize)]
struct TimeSeriesResponse {
    hit_rate: Vec<DataPoint<f64>>,
    latency: Vec<DataPoint<f64>>,
    throughput: Vec<DataPoint<f64>>,
    error_rate: Vec<DataPoint<f64>>,
    cache_size: Vec<DataPoint<u64>>,
    evictions: Vec<DataPoint<u64>>,
}

#[derive(Debug, Serialize)]
struct Alert {
    id: String,
    timestamp: SystemTime,
    severity: AlertSeverity,
    title: String,
    description: String,
    affected_metric: String,
}

#[derive(Debug, Serialize)]
enum AlertSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

#[derive(Debug, Serialize)]
struct AlertsResponse {
    alerts: Vec<Alert>,
}

#[derive(Debug, Serialize)]
struct HeatmapCell {
    hour: u32,
    day: u32,
    value: f64,
    timestamp: SystemTime,
}

#[derive(Debug, Serialize)]
struct HeatmapData {
    cells: Vec<HeatmapCell>,
}

#[derive(Debug, Serialize)]
struct TopKey {
    key: String,
    hits: u64,
    misses: u64,
    size_bytes: u64,
    last_access: SystemTime,
}

#[derive(Debug, Serialize)]
struct TopKeysResponse {
    keys: Vec<TopKey>,
}

/// HTML template for the dashboard (simplified version)
const DASHBOARD_HTML: &str = r#"
<!DOCTYPE html>
<html>
<head>
    <title>Cache Analytics Dashboard</title>
    <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            margin: 0;
            padding: 20px;
            background: #f5f5f5;
        }
        .container {
            max-width: 1400px;
            margin: 0 auto;
        }
        .header {
            background: white;
            padding: 20px;
            border-radius: 8px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
            margin-bottom: 20px;
        }
        .metrics-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 20px;
            margin-bottom: 20px;
        }
        .metric-card {
            background: white;
            padding: 20px;
            border-radius: 8px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }
        .metric-value {
            font-size: 2em;
            font-weight: bold;
            margin: 10px 0;
        }
        .metric-label {
            color: #666;
            font-size: 0.9em;
        }
        .chart-container {
            background: white;
            padding: 20px;
            border-radius: 8px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
            margin-bottom: 20px;
        }
        .status-good { color: #22c55e; }
        .status-warning { color: #f59e0b; }
        .status-error { color: #ef4444; }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>Cache Analytics Dashboard</h1>
            <p>Real-time monitoring and insights</p>
        </div>
        
        <div class="metrics-grid">
            <div class="metric-card">
                <div class="metric-label">Hit Rate</div>
                <div class="metric-value status-good" id="hit-rate">--</div>
            </div>
            <div class="metric-card">
                <div class="metric-label">Avg Latency</div>
                <div class="metric-value" id="latency">--</div>
            </div>
            <div class="metric-card">
                <div class="metric-label">Throughput</div>
                <div class="metric-value" id="throughput">--</div>
            </div>
            <div class="metric-card">
                <div class="metric-label">Error Rate</div>
                <div class="metric-value status-good" id="error-rate">--</div>
            </div>
        </div>
        
        <div class="chart-container">
            <h2>Performance Trends</h2>
            <canvas id="performance-chart"></canvas>
        </div>
        
        <div class="chart-container">
            <h2>Cache Size & Evictions</h2>
            <canvas id="cache-chart"></canvas>
        </div>
    </div>
    
    <script>
        // Initialize charts and start real-time updates
        async function updateMetrics() {
            try {
                const response = await fetch('/api/stats');
                const data = await response.json();
                
                document.getElementById('hit-rate').textContent = 
                    (data.hit_rate * 100).toFixed(1) + '%';
                document.getElementById('latency').textContent = 
                    data.avg_latency.toFixed(1) + 'ms';
                document.getElementById('throughput').textContent = 
                    data.throughput.toFixed(0) + ' ops/s';
                document.getElementById('error-rate').textContent = 
                    (data.error_rate * 100).toFixed(3) + '%';
            } catch (error) {
                console.error('Failed to update metrics:', error);
            }
        }
        
        // Update every 5 seconds
        setInterval(updateMetrics, 5000);
        updateMetrics();
    </script>
</body>
</html>
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dashboard_config() {
        let config = DashboardConfig::default();
        assert_eq!(config.port, 9091);
        assert_eq!(config.retention_period, Duration::from_secs(86400));
    }

    #[tokio::test]
    async fn test_time_series_cleanup() {
        let mut history = VecDeque::new();
        let now = SystemTime::now();

        // Add old and new data points
        history.push_back(DataPoint {
            timestamp: now - Duration::from_secs(3600),
            value: 1.0,
        });
        history.push_back(DataPoint {
            timestamp: now - Duration::from_secs(1800),
            value: 2.0,
        });
        history.push_back(DataPoint {
            timestamp: now,
            value: 3.0,
        });

        // Clean up data older than 30 minutes
        let cutoff = now - Duration::from_secs(1800);
        AnalyticsDashboard::cleanup_old_data(&mut history, cutoff);

        assert_eq!(history.len(), 2);
        assert_eq!(history[0].value, 2.0);
        assert_eq!(history[1].value, 3.0);
    }
}
