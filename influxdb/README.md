# InfluxDB 腐蚀监测配置说明

本目录包含考古腐蚀监测项目的 InfluxDB 配置文件，包括保留策略、连续查询和 Flux 降采样任务。

## 目录结构

```
influxdb/
├── docker-compose.yml    # Docker Compose 配置
├── init.iql              # InfluxQL 初始化脚本 (保留策略 + 连续查询)
├── downsample.flux       # Flux 降采样任务集合 (InfluxDB 2.x)
└── README.md             # 本说明文档
```

## 数据保留层级 (Retention Tiers)

系统采用四级数据保留策略，平衡存储成本和数据可用性：

| 层级 | 保留策略 / Bucket | 保留时长 | 粒度 | 用途 |
|------|-------------------|----------|------|------|
| L0 原始数据 | autogen / corrosion_data | 30 天 | 原始采样 | 实时监控、故障排查、精细分析 |
| L1 分钟级 | rp_1m / corrosion_data_downsampled_1m | 90 天 | 1 分钟 | 短期趋势分析、小时级报表 |
| L2 小时级 | rp_1h / corrosion_data_downsampled_1h | 365 天 | 1 小时 | 中长期趋势分析、月度报表 |
| L3 日级 | rp_1d / corrosion_data_downsampled_1d | 3 年 (1095天) | 1 天 | 长期趋势分析、年度报表、考古研究 |

### 降采样链路

```
原始数据 (autogen)
    ↓ 1分钟聚合
L1 分钟级 (rp_1m)
    ↓ 1小时聚合
L2 小时级 (rp_1h)
    ↓ 每日聚合
L3 日级 (rp_1d)
```

## 测量与字段说明

### metal_corrosion (金属腐蚀监测)

**Tags:**
- `probe_id` - 腐蚀探头编号
- `zone` - 遗址区域
- `material_type` - 材料类型 (iron / copper)

**Fields (原始):**
- `corrosion_rate` - 腐蚀速率 (mm/year)
- `resistance` - 电阻 (ohm)
- `polarization_resistance` - 极化电阻 (ohm-cm²)

**Fields (降采样后):**
- `mean_corrosion_rate` - 平均腐蚀速率
- `min_corrosion_rate` - 最小腐蚀速率
- `max_corrosion_rate` - 最大腐蚀速率
- `last_corrosion_rate` - 最新腐蚀速率
- `mean_resistance` - 平均电阻
- `mean_polarization_resistance` - 平均极化电阻

### soil_environment (土壤环境监测)

**Tags:**
- `sensor_id` - 传感器编号
- `zone` - 遗址区域
- `sensor_type` - 传感器类型

**Fields (原始):**
- `temperature` - 温度 (°C)
- `humidity` - 湿度 (%)
- `ph` - pH 值
- `chloride` - 氯离子含量 (ppm)

**Fields (降采样后):**
- `mean_temperature` - 平均温度
- `mean_humidity` - 平均湿度
- `mean_ph` - 平均 pH 值
- `mean_chloride` - 平均氯离子含量

## 配置方式选择

项目提供两种降采样配置方式，根据你的 InfluxDB 版本和使用习惯选择：

### 方式一: InfluxQL 连续查询 (init.iql)

适用于 InfluxDB 1.x 或 2.x (通过 InfluxQL 兼容层)。

**包含的连续查询:**

| CQ 名称 | 源数据 | 目标 | 频率 |
|---------|--------|------|------|
| `cq_corrosion_1m` | autogen.metal_corrosion | rp_1m.corrosion_1m | 每分钟 |
| `cq_soil_1m` | autogen.soil_environment | rp_1m.soil_1m | 每分钟 |
| `cq_corrosion_1h` | rp_1m.corrosion_1m | rp_1h.corrosion_1h | 每小时 |
| `cq_soil_1h` | rp_1m.soil_1m | rp_1h.soil_1h | 每小时 |
| `cq_corrosion_1d` | rp_1h.corrosion_1h | rp_1d.corrosion_1d | 每天 |
| `cq_soil_1d` | rp_1h.soil_1h | rp_1d.soil_1d | 每天 |

**使用方法:**

1. 确保 InfluxDB 已启动
2. 使用 influx CLI 执行:
   ```bash
   influx -database corrosion_data -import -path init.iql
   ```
3. 或通过 Docker 自动执行 (docker-compose.yml 已挂载)

### 方式二: Flux 任务 (downsample.flux)

适用于 InfluxDB 2.x，是 InfluxDB 2.x 的推荐方式。

**包含的 Flux 任务:**

| 任务名称 | 源 Bucket | 目标 Bucket | 频率 | 聚合函数 |
|---------|-----------|-------------|------|----------|
| `downsample_1m` | corrosion_data | corrosion_data_downsampled_1m | 每分钟 | mean |
| `downsample_1h` | corrosion_data_downsampled_1m | corrosion_data_downsampled_1h | 每小时 | mean, median, stddev |
| `downsample_1d` | corrosion_data_downsampled_1h | corrosion_data_downsampled_1d | 每天 | mean, min, max |

**使用方法:**

1. 创建降采样 Bucket:
   ```bash
   influx bucket create --name corrosion_data_downsampled_1m --org archaeology --retention 90d
   influx bucket create --name corrosion_data_downsampled_1h --org archaeology --retention 365d
   influx bucket create --name corrosion_data_downsampled_1d --org archaeology --retention 1095d
   ```

2. 通过 UI 创建任务:
   - 打开 InfluxDB UI (http://localhost:8086)
   - 进入 Tasks → Create Task → New Task
   - 复制对应任务的 Flux 代码
   - 保存并启用

3. 或通过 CLI 创建任务:
   ```bash
   # 将 downsample.flux 中对应任务的代码提取为单独文件后执行
   influx task create --name "downsample_1m" --file task_1m.flux
   ```

## 验证方法

### 验证保留策略

**InfluxQL:**
```sql
-- 查看所有保留策略
SHOW RETENTION POLICIES ON "corrosion_data";
```

**Flux:**
```flux
// 查看所有 bucket
import "influxdata/influxdb/v1"
v1.databases()
```

或使用 CLI:
```bash
influx bucket list --org archaeology
```

### 验证连续查询 / 任务

**InfluxQL:**
```sql
-- 查看所有连续查询
SHOW CONTINUOUS QUERIES;
```

**Flux 任务:**
```bash
influx task list --org archaeology
```

### 验证数据写入

**验证 1分钟降采样数据:**
```sql
-- InfluxQL
SELECT * FROM "rp_1m"."corrosion_1m" LIMIT 10;
SELECT * FROM "rp_1m"."soil_1m" LIMIT 10;
```

```flux
// Flux
from(bucket: "corrosion_data_downsampled_1m")
  |> range(start: -1h)
  |> filter(fn: (r) => r._measurement == "metal_corrosion")
  |> limit(n: 10)
```

**验证 1小时降采样数据:**
```sql
-- InfluxQL
SELECT * FROM "rp_1h"."corrosion_1h" LIMIT 10;
SELECT * FROM "rp_1h"."soil_1h" LIMIT 10;
```

**验证每日聚合数据:**
```sql
-- InfluxQL
SELECT * FROM "rp_1d"."corrosion_1d" LIMIT 10;
SELECT * FROM "rp_1d"."soil_1d" LIMIT 10;
```

### 验证数据量

```sql
-- 统计各保留策略下的数据点数量
SELECT count("corrosion_rate") FROM "autogen"."metal_corrosion";
SELECT count("mean_corrosion_rate") FROM "rp_1m"."corrosion_1m";
SELECT count("mean_corrosion_rate") FROM "rp_1h"."corrosion_1h";
SELECT count("mean_corrosion_rate") FROM "rp_1d"."corrosion_1d";
```

### 检查任务运行状态

```bash
# 查看任务列表
influx task list --org archaeology

# 查看任务运行日志
influx task runs list --task-id <task-id> --org archaeology

# 查看最近一次运行详情
influx task run find --task-id <task-id> --run-id <run-id> --org archaeology
```

## 常见问题

### Q: 连续查询不生效怎么办？

A: 
1. 检查 CQ 是否正确创建: `SHOW CONTINUOUS QUERIES`
2. 检查源测量是否有数据写入
3. 检查 CQ 的时间范围是否正确
4. 查看 InfluxDB 日志中的错误信息

### Q: Flux 任务失败了怎么排查？

A:
1. 查看任务运行日志: `influx task runs list --task-id <id>`
2. 检查源 Bucket 是否有数据
3. 验证 Flux 语法: 在 Data Explorer 中测试查询
4. 确保目标 Bucket 已创建且有写入权限

### Q: 如何调整保留策略时长？

A:

**InfluxQL:**
```sql
ALTER RETENTION POLICY "rp_1m" ON "corrosion_data" DURATION 180d;
```

**Flux / CLI:**
```bash
influx bucket update --id <bucket-id> --retention 180d
```

### Q: 数据降采样后体积减少多少？

A: 
- 原始 → 1分钟: 根据采样频率，通常减少 60-90%
- 1分钟 → 1小时: 减少约 98.3% (60倍)
- 1小时 → 1天: 减少约 95.8% (24倍)
- 整体: 原始数据的 0.1% 以下

## 相关文件

- [docker-compose.yml](./docker-compose.yml) - Docker 部署配置
- [init.iql](./init.iql) - InfluxQL 初始化脚本
- [downsample.flux](./downsample.flux) - Flux 降采样任务集合
