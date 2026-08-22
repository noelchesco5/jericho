use serde::{Deserialize, Serialize};
use sysinfo::System;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

// ============================================================
// SYSTEM HEALTH MONITOR
// Tracks: RAM, CPU, disk, GPU/VRAM (if available), process stats
// ============================================================

/// Complete system health snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    pub timestamp: DateTime<Utc>,
    pub ram: RamStats,
    pub cpu: CpuStats,
    pub disk: DiskStats,
    pub process: ProcessStats,
    pub gpu: Option<GpuStats>,
    pub ollama_process: Option<OllamaProcessInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RamStats {
    pub total_mb: u64,
    pub used_mb: u64,
    pub free_mb: u64,
    pub usage_percent: f32,
    pub available_mb: u64,
    /// How much Jericho is using (ollama + this process)
    pub jericho_used_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuStats {
    pub usage_percent: f32,
    pub core_count: usize,
    pub brand: String,
    pub frequency_mhz: u64,
    pub per_core_usage: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskStats {
    pub total_gb: f64,
    pub used_gb: f64,
    pub free_gb: f64,
    pub usage_percent: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessStats {
    pub pid: u32,
    pub name: String,
    pub memory_mb: u64,
    pub cpu_percent: f32,
    pub uptime_secs: u64,
    pub thread_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuStats {
    pub name: String,
    pub vram_total_mb: u64,
    pub vram_used_mb: u64,
    pub vram_free_mb: u64,
    pub usage_percent: f32,
    pub temperature_c: Option<f32>,
    pub driver: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaProcessInfo {
    pub running: bool,
    pub pid: Option<u32>,
    pub memory_mb: u64,
    pub cpu_percent: f32,
    pub thread_count: usize,
}

/// Historical data point for graphs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthSnapshot {
    pub timestamp: DateTime<Utc>,
    pub ram_percent: f32,
    pub cpu_percent: f32,
    pub jericho_mb: u64,
    pub gpu_percent: Option<f32>,
}

/// The main system monitor - samples and caches health data
pub struct SystemMonitor {
    sys: System,
    last_health: SystemHealth,
    /// Ring buffer of historical snapshots for graphing
    history: Vec<HealthSnapshot>,
    max_history: usize,
    /// Resource limits from config
    ram_limit_mb: u64,
    cpu_limit_percent: f32,
}

impl SystemMonitor {
    pub fn new(ram_limit_mb: u64, cpu_limit_percent: f32) -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();

        Self {
            sys,
            last_health: Self::empty_health(),
            history: Vec::new(),
            max_history: 300, // 5 min at 1 sample/sec
            ram_limit_mb,
            cpu_limit_percent,
        }
    }

    /// Refresh all system data and return current health
    pub fn refresh(&mut self) -> SystemHealth {
        self.sys.refresh_all();

        let ram = self.get_ram_stats();
        let cpu = self.get_cpu_stats();
        let disk = self.get_disk_stats();
        let process = self.get_process_stats();
        let ollama = self.get_ollama_info();
        let gpu = self.get_gpu_info();

        let health = SystemHealth {
            timestamp: Utc::now(),
            ram,
            cpu,
            disk,
            process,
            gpu,
            ollama_process: ollama,
        };

        // Add to history
        let snapshot = HealthSnapshot {
            timestamp: Utc::now(),
            ram_percent: health.ram.usage_percent,
            cpu_percent: health.cpu.usage_percent,
            jericho_mb: health.ram.jericho_used_mb,
            gpu_percent: health.gpu.as_ref().map(|g| g.usage_percent),
        };
        self.history.push(snapshot);
        if self.history.len() > self.max_history {
            self.history.remove(0);
        }

        self.last_health = health.clone();
        health
    }

    /// Check if any resource exceeds configured limits
    pub fn check_throttle(&self) -> Vec<ThrottleAlert> {
        let mut alerts = Vec::new();
        let h = &self.last_health;

        if h.ram.usage_percent > 90.0 {
            alerts.push(ThrottleAlert {
                severity: AlertSeverity::Critical,
                resource: "RAM".to_string(),
                message: format!("RAM at {:.1}% - limit {}MB", h.ram.usage_percent, self.ram_limit_mb),
                current: h.ram.used_mb as f64,
                limit: self.ram_limit_mb as f64,
            });
        } else if h.ram.usage_percent > 80.0 {
            alerts.push(ThrottleAlert {
                severity: AlertSeverity::Warning,
                resource: "RAM".to_string(),
                message: format!("RAM at {:.1}%", h.ram.usage_percent),
                current: h.ram.used_mb as f64,
                limit: self.ram_limit_mb as f64,
            });
        }

        if h.cpu.usage_percent > self.cpu_limit_percent {
            alerts.push(ThrottleAlert {
                severity: AlertSeverity::Warning,
                resource: "CPU".to_string(),
                message: format!(
                    "CPU at {:.1}% (limit {:.0}%)",
                    h.cpu.usage_percent, self.cpu_limit_percent
                ),
                current: h.cpu.usage_percent as f64,
                limit: self.cpu_limit_percent as f64,
            });
        }

        alerts
    }

    pub fn get_history(&self) -> &[HealthSnapshot] {
        &self.history
    }

    pub fn get_last_health(&self) -> &SystemHealth {
        &self.last_health
    }

    pub fn update_limits(&mut self, ram_mb: u64, cpu_percent: f32) {
        self.ram_limit_mb = ram_mb;
        self.cpu_limit_percent = cpu_percent;
    }

    // ---- Internal sampling methods ----

    fn get_ram_stats(&mut self) -> RamStats {
        let total = self.sys.total_memory();
        let used = self.sys.used_memory();

        let total_mb = total / 1_048_576;
        let used_mb = used / 1_048_576;
        let free_mb = total_mb - used_mb;
        let usage_percent = (used_mb as f32 / total_mb as f32) * 100.0;

        // Calculate Jericho-specific usage
        let mut jericho_mb = 0u64;
        for (_pid, proc_) in self.sys.processes() {
            let name = proc_.name().to_string_lossy().to_lowercase();
            if name.contains("ollama") || name.contains("project_jericho") || name.contains("jericho") {
                jericho_mb += proc_.memory() / 1_048_576;
            }
        }

        RamStats {
            total_mb,
            used_mb,
            free_mb,
            usage_percent,
            available_mb: free_mb,
            jericho_used_mb: jericho_mb,
        }
    }

    fn get_cpu_stats(&mut self) -> CpuStats {
        let usage = self.sys.global_cpu_usage();
        let cores = self.sys.cpus();
        let per_core: Vec<f32> = cores.iter().map(|c| c.cpu_usage()).collect();
        let brand = cores.first().map(|c| c.brand().to_string()).unwrap_or_default();
        let freq = cores.first().map(|c| c.frequency()).unwrap_or(0);

        CpuStats {
            usage_percent: usage,
            core_count: cores.len(),
            brand,
            frequency_mhz: freq,
            per_core_usage: per_core,
        }
    }

    fn get_disk_stats(&self) -> DiskStats {
        let disks = sysinfo::Disks::new_with_refreshed_list();
        let mut total: u64 = 0;
        let mut available: u64 = 0;

        for disk in disks.iter() {
            total += disk.total_space();
            available += disk.available_space();
        }

        let total_gb = total as f64 / 1_073_741_824.0;
        let free_gb = available as f64 / 1_073_741_824.0;
        let used_gb = total_gb - free_gb;
        let usage_percent = if total_gb > 0.0 { (used_gb / total_gb) as f32 * 100.0 } else { 0.0 };

        DiskStats {
            total_gb,
            used_gb,
            free_gb,
            usage_percent,
        }
    }

    fn get_process_stats(&self) -> ProcessStats {
        let pid = std::process::id();
        if let Some(proc_) = self.sys.process(sysinfo::Pid::from_u32(pid)) {
            ProcessStats {
                pid,
                name: proc_.name().to_string_lossy().to_string(),
                memory_mb: proc_.memory() / 1_048_576,
                cpu_percent: proc_.cpu_usage(),
                uptime_secs: proc_.run_time(),
                thread_count: 0,
            }
        } else {
            ProcessStats {
                pid,
                name: "project-jericho".to_string(),
                memory_mb: 0,
                cpu_percent: 0.0,
                uptime_secs: 0,
                thread_count: 0,
            }
        }
    }

    fn get_ollama_info(&self) -> Option<OllamaProcessInfo> {
        for (pid, proc_) in self.sys.processes() {
            let name = proc_.name().to_string_lossy().to_lowercase();
            if name.contains("ollama") {
                return Some(OllamaProcessInfo {
                    running: true,
                    pid: Some(pid.as_u32()),
                    memory_mb: proc_.memory() / 1_048_576,
                    cpu_percent: proc_.cpu_usage(),
                    thread_count: 0,
                });
            }
        }
        Some(OllamaProcessInfo {
            running: false,
            pid: None,
            memory_mb: 0,
            cpu_percent: 0.0,
            thread_count: 0,
        })
    }

    fn get_gpu_info(&self) -> Option<GpuStats> {
        // On Windows, try WMI-based detection or nvidia-smi
        // For now, return None - GPU detection is platform-specific
        // Can be expanded with nvml-wrapper crate later
        None
    }

    fn empty_health() -> SystemHealth {
        SystemHealth {
            timestamp: Utc::now(),
            ram: RamStats { total_mb: 0, used_mb: 0, free_mb: 0, usage_percent: 0.0, available_mb: 0, jericho_used_mb: 0 },
            cpu: CpuStats { usage_percent: 0.0, core_count: 0, brand: String::new(), frequency_mhz: 0, per_core_usage: Vec::new() },
            disk: DiskStats { total_gb: 0.0, used_gb: 0.0, free_gb: 0.0, usage_percent: 0.0 },
            process: ProcessStats { pid: 0, name: String::new(), memory_mb: 0, cpu_percent: 0.0, uptime_secs: 0, thread_count: 0 },
            gpu: None,
            ollama_process: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThrottleAlert {
    pub severity: AlertSeverity,
    pub resource: String,
    pub message: String,
    pub current: f64,
    pub limit: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

/// Shared monitor for concurrent access
pub type SharedMonitor = Arc<RwLock<SystemMonitor>>;
