use std::time::Duration;
use chrono::Utc;
use clap::Parser;
use rand::{rngs::StdRng, SeedableRng, Rng};
use reqwest::Client;
use serde::Serialize;
use tracing::{info, warn, error};
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Parser, Debug)]
#[command(name = "LoRa Simulator", version, about = "古代战地医院遗址腐蚀监测系统 LoRa 数据模拟器")]
struct Args {
    #[arg(long, env = "SIM_ENDPOINT", default_value = "http://localhost:8080/api/lora/data")]
    endpoint: String,

    #[arg(long, env = "SIM_INTERVAL_MINUTES", default_value_t = 30)]
    interval_minutes: u64,

    #[arg(long, env = "SIM_SOIL_SENSORS", default_value_t = 40)]
    soil_sensors: usize,

    #[arg(long, env = "SIM_CORROSION_PROBES", default_value_t = 20)]
    corrosion_probes: usize,

    #[arg(long, env = "SIM_BURST_MODE", default_value_t = false)]
    burst_mode: bool,

    #[arg(long, env = "SIM_RANDOM_SEED", default_value_t = 42)]
    random_seed: u64,

    #[arg(long, env = "SIM_CHLORIDE_SPIKE_ENABLED", default_value_t = false)]
    chloride_spike_enabled: bool,

    #[arg(long, env = "SIM_CHLORIDE_SPIKE_ZONE", default_value = "区域1-主营区")]
    chloride_spike_zone: String,

    #[arg(long, env = "SIM_CHLORIDE_SPIKE_VALUE", default_value_t = 300.0)]
    chloride_spike_value: f64,

    #[arg(long, env = "SIM_CHLORIDE_SPIKE_DURATION_HOURS", default_value_t = 6)]
    chloride_spike_duration_hours: u64,

    #[arg(long, env = "SIM_CHLORIDE_SPIKE_INTERVAL_HOURS", default_value_t = 48)]
    chloride_spike_interval_hours: u64,
}

#[derive(Serialize)]
struct LoraPacket {
    device_type: String,
    device_id: String,
    zone: String,
    seq_id: u64,
    timestamp: String,
    data: LoraData,
}

#[derive(Serialize)]
#[serde(untagged)]
enum LoraData {
    Soil(SoilReading),
    Corrosion(CorrosionReading),
}

#[derive(Serialize)]
struct SoilReading {
    temperature: f64,
    humidity: f64,
    ph: f64,
    chloride: f64,
}

#[derive(Serialize)]
struct CorrosionReading {
    material_type: String,
    resistance: f64,
    polarization_resistance: f64,
}

const ZONE_NAMES: &[&str] = &[
    "区域1-主营区",
    "区域2-东翼",
    "区域3-西翼",
    "区域4-南侧",
    "区域5-北侧",
];

fn zone_name(idx: usize) -> String {
    ZONE_NAMES[idx % ZONE_NAMES.len()].to_string()
}

fn zone_for_soil(idx: usize) -> String {
    zone_name((idx / 8) % 5)
}

fn zone_for_corrosion(idx: usize) -> String {
    zone_name((idx / 5) % 5)
}

fn zone_index_for_soil(idx: usize) -> usize {
    (idx / 8) % 5
}

fn get_adjacent_zones(target_zone: &str) -> Vec<usize> {
    let target_idx = ZONE_NAMES.iter().position(|z| *z == target_zone).unwrap_or(0);
    let mut adjacent = Vec::new();
    if target_idx > 0 {
        adjacent.push(target_idx - 1);
    }
    if target_idx < ZONE_NAMES.len() - 1 {
        adjacent.push(target_idx + 1);
    }
    adjacent
}

fn is_spike_active(elapsed_hours: f64, interval_hours: u64, duration_hours: u64) -> bool {
    let cycle = elapsed_hours % interval_hours as f64;
    cycle < duration_hours as f64
}

fn generate_soil_data(sensor_idx: usize, rng: &mut StdRng, spike_enabled: bool, spike_zone_idx: usize, spike_value: f64, spike_active: bool) -> SoilReading {
    let zone_idx = zone_index_for_soil(sensor_idx);
    let zone = zone_idx;
    let temp_base = match zone {
        0 => 14.0,
        1 => 16.5,
        2 => 18.0,
        3 => 15.5,
        _ => 20.0,
    };

    let hum_base = match zone {
        0 => 45.0,
        1 => 55.0,
        2 => 65.0,
        3 => 50.0,
        _ => 70.0,
    };

    let cl_base = match zone {
        0 => 35.0,
        1 => 50.0,
        2 => 85.0,
        3 => 40.0,
        _ => 120.0,
    };

    let mut chloride = f64::max(cl_base + rng.gen_range(-15.0_f64..30.0_f64), 5.0);

    if spike_enabled && spike_active {
        let adjacent_zones = get_adjacent_zones(&ZONE_NAMES[spike_zone_idx]);
        if zone_idx == spike_zone_idx {
            chloride = spike_value;
        } else if adjacent_zones.contains(&zone_idx) {
            chloride = (cl_base + spike_value) / 2.0;
        }
    }

    SoilReading {
        temperature: temp_base + rng.gen_range(-3.0..3.0),
        humidity: hum_base + rng.gen_range(-10.0..10.0),
        ph: 6.8 + rng.gen_range(-1.2..1.2),
        chloride,
    }
}

fn generate_corrosion_data(probe_idx: usize, rng: &mut StdRng) -> CorrosionReading {
    let material_type = if probe_idx % 2 == 0 { "iron".to_string() } else { "copper".to_string() };

    let zone = (probe_idx / 5) % 5;
    let (rp_base, r_base) = match (zone, material_type.as_str()) {
        (2, "iron") => (45.0, 120.0),
        (4, "iron") => (35.0, 100.0),
        (_, "iron") => (80.0, 180.0),
        (2, "copper") => (120.0, 250.0),
        (4, "copper") => (90.0, 200.0),
        (_, "copper") => (180.0, 350.0),
        _ => (80.0, 180.0),
    };

    CorrosionReading {
        material_type,
        resistance: r_base + rng.gen_range(-20.0..20.0),
        polarization_resistance: f64::max(rp_base + rng.gen_range(-15.0_f64..25.0_f64), 20.0),
    }
}

async fn send_packet(client: &Client, url: &str, packet: &LoraPacket) -> bool {
    match client
        .post(url)
        .json(packet)
        .timeout(Duration::from_secs(10))
        .send()
        .await
    {
        Ok(resp) => {
            if resp.status().is_success() {
                true
            } else {
                warn!("发送失败, HTTP状态: {}", resp.status());
                false
            }
        }
        Err(e) => {
            error!("请求错误: {}", e);
            false
        }
    }
}

#[tokio::main]
async fn main() {
    fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let args = Args::parse();

    info!("LoRa 数据模拟器启动");
    info!("目标端点: {}", args.endpoint);
    info!("土壤传感器: {} 台", args.soil_sensors);
    info!("腐蚀探头: {} 台", args.corrosion_probes);
    info!("随机种子: {}", args.random_seed);
    if args.burst_mode {
        info!("模式: 突发模式 (一次性发送全部数据)");
    } else {
        info!("模式: 定时模式 (每 {} 分钟)", args.interval_minutes);
    }

    if args.chloride_spike_enabled {
        info!("氯化物尖峰注入: 已启用");
        info!("  目标区域: {}", args.chloride_spike_zone);
        info!("  尖峰浓度: {:.1} ppm", args.chloride_spike_value);
        info!("  持续时间: {} 小时", args.chloride_spike_duration_hours);
        info!("  间隔周期: {} 小时", args.chloride_spike_interval_hours);
    } else {
        info!("氯化物尖峰注入: 已禁用");
    }

    let spike_zone_idx = ZONE_NAMES.iter().position(|z| *z == args.chloride_spike_zone).unwrap_or(0);

    let mut rng = StdRng::seed_from_u64(args.random_seed);
    let client = Client::new();
    let mut soil_seq: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let mut corrosion_seq: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    let mut cycle_count: u64 = 0;
    let mut elapsed_hours: f64 = 0.0;
    let mut prev_spike_active = false;

    loop {
        cycle_count += 1;
        let spike_active = if args.chloride_spike_enabled {
            is_spike_active(elapsed_hours, args.chloride_spike_interval_hours, args.chloride_spike_duration_hours)
        } else {
            false
        };

        if args.chloride_spike_enabled {
            if spike_active && !prev_spike_active {
                info!("===== 氯化物尖峰事件开始 =====");
                info!("目标区域: {}, 浓度: {:.1} ppm", args.chloride_spike_zone, args.chloride_spike_value);
                let adjacent = get_adjacent_zones(&args.chloride_spike_zone);
                info!("相邻区域 (50% 效果): {:?}", adjacent.iter().map(|i| ZONE_NAMES[*i]).collect::<Vec<_>>());
            } else if !spike_active && prev_spike_active {
                info!("===== 氯化物尖峰事件结束 =====");
            }
        }

        info!("===== 第 {} 轮数据上报 =====", cycle_count);
        info!("已运行时长: {:.2} 小时", elapsed_hours);
        info!("氯化物尖峰状态: {}", if spike_active { "激活中" } else { "未激活" });

        let mut success_count = 0;
        let mut fail_count = 0;

        for i in 0..args.soil_sensors {
            let device_id = format!("SOIL-{:03}", i + 1);
            let seq = soil_seq.entry(device_id.clone()).and_modify(|s| *s += 1).or_insert(1);
            let packet = LoraPacket {
                device_type: "soil_sensor".to_string(),
                device_id: device_id.clone(),
                zone: zone_for_soil(i),
                seq_id: *seq,
                timestamp: Utc::now().to_rfc3339(),
                data: LoraData::Soil(generate_soil_data(i, &mut rng, args.chloride_spike_enabled, spike_zone_idx, args.chloride_spike_value, spike_active)),
            };

            if send_packet(&client, &args.endpoint, &packet).await {
                success_count += 1;
            } else {
                fail_count += 1;
            }

            if !args.burst_mode {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }

        for i in 0..args.corrosion_probes {
            let device_id = format!("CORR-{:03}", i + 1);
            let seq = corrosion_seq.entry(device_id.clone()).and_modify(|s| *s += 1).or_insert(1);
            let packet = LoraPacket {
                device_type: "corrosion_probe".to_string(),
                device_id: device_id.clone(),
                zone: zone_for_corrosion(i),
                seq_id: *seq,
                timestamp: Utc::now().to_rfc3339(),
                data: LoraData::Corrosion(generate_corrosion_data(i, &mut rng)),
            };

            if send_packet(&client, &args.endpoint, &packet).await {
                success_count += 1;
            } else {
                fail_count += 1;
            }

            if !args.burst_mode {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }

        info!(
            "本轮上报完成: 成功 {}, 失败 {}, 总数 {}",
            success_count, fail_count, success_count + fail_count
        );

        if args.burst_mode {
            info!("突发模式完成，退出程序");
            break;
        }

        prev_spike_active = spike_active;
        elapsed_hours += args.interval_minutes as f64 / 60.0;

        info!("等待 {} 分钟后进行下一轮上报...", args.interval_minutes);
        tokio::time::sleep(Duration::from_secs(args.interval_minutes * 60)).await;
    }
}
