const MB: u64 = 1024 * 1024;
const MEM_DELTA_PP: f32 = 5.0;
const CPU_THRESHOLD_PCT: f32 = 50.0;

/// Spawns a background task that samples memory, swap and CPU every 10s and
/// only emits a log line when usage crosses a threshold (memory used-% delta
/// > 5pp from the last logged sample, or CPU > 50%). The first sample is
/// always logged to give a baseline; otherwise an idle session would never
/// emit, leaving no resource context in error reports.
pub fn spawn_sysinfo_logger() {
    tauri::async_runtime::spawn(async move {
        let mut system = sysinfo::System::new();
        // Prime the CPU sampler: sysinfo needs two refreshes separated by at
        // least MINIMUM_CPU_UPDATE_INTERVAL (~200ms) to compute a usage delta,
        // otherwise the first reading is always 0.0%.
        system.refresh_cpu_all();
        tokio::time::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL).await;
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut last_logged_mem_pct: Option<f32> = None;
        loop {
            interval.tick().await;
            system.refresh_memory();
            system.refresh_cpu_all();

            let used_mem = system.used_memory();
            let total_mem = system.total_memory();
            let mem_pct = if total_mem > 0 {
                used_mem as f32 / total_mem as f32 * 100.0
            } else {
                0.0
            };
            let cpu = system.global_cpu_usage();

            let mem_crossed_threshold =
                last_logged_mem_pct.map_or(true, |last| (mem_pct - last).abs() > MEM_DELTA_PP);
            let cpu_high = cpu > CPU_THRESHOLD_PCT;
            if !mem_crossed_threshold && !cpu_high {
                continue;
            }

            log::info!(
                "SysInfo: mem {}/{} MB ({:.1}%), swap {}/{} MB, cpu {:.1}%",
                used_mem / MB,
                total_mem / MB,
                mem_pct,
                system.used_swap() / MB,
                system.total_swap() / MB,
                cpu,
            );
            last_logged_mem_pct = Some(mem_pct);
        }
    });
}
