const API_BASE = window.location.origin.includes('localhost') && !window.location.origin.includes('8080')
    ? 'http://localhost:8080/api'
    : '/api';

let map;
let heatmapCanvas;
let heatmapCtx;
let deviceMarkers = [];
let soilLayer = L.layerGroup();
let corrosionLayer = L.layerGroup();
let allLocations = [];
let currentHeatmapData = [];
let showHeatmap = true;
let showSoil = true;
let showCorrosion = true;

function init() {
    initMap();
    heatmapCanvas = document.getElementById('heatmap-canvas');
    heatmapCtx = heatmapCanvas.getContext('2d');

    loadLocations();
    loadStats();
    loadHeatmap();
    bindEvents();
    window.addEventListener('resize', resizeCanvas);
    resizeCanvas();
}

function initMap() {
    map = L.map('map', {
        center: [34.2658, 108.9542],
        zoom: 18,
        minZoom: 16,
        maxZoom: 22,
        zoomControl: true,
    });

    L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
        attribution: '&copy; OpenStreetMap',
        maxZoom: 22,
    }).addTo(map);

    const siteBounds = L.latLngBounds(
        [34.2644, 108.9527],
        [34.2672, 108.9557]
    );

    const siteRect = L.rectangle(siteBounds, {
        color: '#fbbf24',
        weight: 2,
        fillColor: '#fbbf24',
        fillOpacity: 0.05,
        dashArray: '8, 4',
    }).addTo(map);

    siteRect.bindTooltip('宋代战地医院遗址 (2,000㎡)', {
        permanent: false,
        direction: 'top',
        offset: [0, -5],
        className: 'site-label',
    });

    L.marker([34.2658, 108.9542], {
        icon: L.divIcon({
            className: 'site-center',
            html: '<div style="text-align:center;color:#fbbf24;font-weight:bold;font-size:11px;">🏥 遗址中心</div>',
            iconSize: [80, 20],
            iconAnchor: [40, 10],
        })
    }).addTo(map);

    soilLayer.addTo(map);
    corrosionLayer.addTo(map);

    map.on('moveend zoomend', () => {
        resizeCanvas();
        renderHeatmap();
    });
}

function resizeCanvas() {
    const container = document.querySelector('.map-section');
    if (!container || !heatmapCanvas) return;
    heatmapCanvas.width = container.clientWidth;
    heatmapCanvas.height = container.clientHeight;
    renderHeatmap();
}

function bindEvents() {
    document.getElementById('close-modal').addEventListener('click', closeModal);
    document.getElementById('detail-modal').addEventListener('click', (e) => {
        if (e.target.id === 'detail-modal') closeModal();
    });

    document.querySelectorAll('.tab-btn').forEach(btn => {
        btn.addEventListener('click', () => {
            document.querySelectorAll('.tab-btn').forEach(b => b.classList.remove('active'));
            btn.classList.add('active');
            filterDeviceList(btn.dataset.tab);
        });
    });

    document.getElementById('toggle-heatmap').addEventListener('change', (e) => {
        showHeatmap = e.target.checked;
        if (!showHeatmap) {
            heatmapCtx.clearRect(0, 0, heatmapCanvas.width, heatmapCanvas.height);
        } else {
            renderHeatmap();
        }
    });

    document.getElementById('toggle-soil').addEventListener('change', (e) => {
        showSoil = e.target.checked;
        if (showSoil) map.addLayer(soilLayer);
        else map.removeLayer(soilLayer);
    });

    document.getElementById('toggle-corrosion').addEventListener('change', (e) => {
        showCorrosion = e.target.checked;
        if (showCorrosion) map.addLayer(corrosionLayer);
        else map.removeLayer(corrosionLayer);
    });

    document.getElementById('heatmap-range').addEventListener('change', loadHeatmap);
    document.getElementById('btn-refresh').addEventListener('click', () => {
        loadStats();
        loadHeatmap();
    });
}

async function apiGet(path) {
    try {
        const res = await fetch(`${API_BASE}${path}`);
        return await res.json();
    } catch (e) {
        console.error('API Error:', path, e);
        return { success: false, data: null, message: e.message };
    }
}

async function loadLocations() {
    const result = await apiGet('/locations');
    if (result.success && result.data) {
        allLocations = result.data;
        renderDeviceList('all');
        renderMarkers();
    }
}

async function loadStats() {
    const result = await apiGet('/stats');
    if (result.success && result.data) {
        const s = result.data;
        document.getElementById('stat-total').textContent = s.total_devices || '60';
        document.getElementById('stat-risk').textContent = s.high_risk_probes || '--';
        document.getElementById('stat-avg').textContent = (s.avg_corrosion_rate || '--') + ' mm/y';
    }
}

async function loadHeatmap() {
    const hours = document.getElementById('heatmap-range').value;
    const result = await apiGet(`/corrosion/heatmap?hours=${hours}`);
    if (result.success && result.data) {
        currentHeatmapData = result.data;
        renderHeatmap();
    }
}

function renderDeviceList(tab) {
    const list = document.getElementById('device-list');
    let filtered = allLocations;

    if (tab === 'soil') {
        filtered = allLocations.filter(l => l.device_type === 'soil_sensor');
    } else if (tab === 'corrosion') {
        filtered = allLocations.filter(l => l.device_type === 'corrosion_probe');
    }

    list.innerHTML = filtered.map(loc => {
        const dotClass = loc.device_type === 'soil_sensor' ? 'soil'
            : (loc.material_type === 'iron' ? 'iron' : 'copper');
        return `
            <div class="device-item" data-id="${loc.id}">
                <div class="device-dot ${dotClass}"></div>
                <div class="device-info">
                    <div class="device-name">${loc.name}</div>
                    <div class="device-zone">${loc.zone} · ${loc.id}</div>
                </div>
            </div>
        `;
    }).join('');

    list.querySelectorAll('.device-item').forEach(item => {
        item.addEventListener('click', () => openDeviceDetail(item.dataset.id));
    });
}

function filterDeviceList(tab) {
    renderDeviceList(tab);
}

function renderMarkers() {
    soilLayer.clearLayers();
    corrosionLayer.clearLayers();
    deviceMarkers = [];

    allLocations.forEach(loc => {
        if (loc.device_type === 'soil_sensor') {
            const icon = createSensorIcon('soil');
            const marker = L.marker([loc.lat, loc.lng], { icon });
            marker.bindPopup(createPopupHtml(loc));
            marker.on('click', () => {
                map.setView([loc.lat, loc.lng], 19);
            });
            marker.addTo(soilLayer);
            deviceMarkers.push({ id: loc.id, marker });
        } else {
            const matType = loc.material_type === 'copper' ? 'copper' : 'iron';
            const icon = createSensorIcon(matType);
            const marker = L.marker([loc.lat, loc.lng], { icon });
            marker.bindPopup(createPopupHtml(loc));
            marker.on('click', () => {
                openDeviceDetail(loc.id);
            });
            marker.addTo(corrosionLayer);
            deviceMarkers.push({ id: loc.id, marker });
        }
    });
}

function createSensorIcon(type) {
    const colors = {
        soil: '#22c55e',
        iron: '#ef4444',
        copper: '#f59e0b',
    };
    const color = colors[type] || '#3b82f6';

    return L.divIcon({
        className: 'custom-marker',
        html: `
            <div style="
                width: 14px; height: 14px;
                background: ${color};
                border: 2px solid white;
                border-radius: 50%;
                box-shadow: 0 0 10px ${color}, 0 2px 6px rgba(0,0,0,0.5);
            "></div>
        `,
        iconSize: [14, 14],
        iconAnchor: [7, 7],
    });
}

function createPopupHtml(loc) {
    if (loc.device_type === 'soil_sensor') {
        return `
            <div>
                <div class="popup-title">🌱 ${loc.name}</div>
                <div class="popup-info">
                    设备ID: ${loc.id}<br>
                    所在区域: ${loc.zone}<br>
                    类型: 温湿度/pH/氯离子传感器
                </div>
            </div>
        `;
    } else {
        const matName = loc.material_type === 'iron' ? '铁器' : '铜器';
        return `
            <div>
                <div class="popup-title">⚙️ ${loc.name}</div>
                <div class="popup-info">
                    设备ID: ${loc.id}<br>
                    所在区域: ${loc.zone}<br>
                    监测对象: ${matName}<br>
                    <a class="popup-btn" onclick="openDeviceDetail('${loc.id}')">查看详情</a>
                </div>
            </div>
        `;
    }
}

function renderHeatmap() {
    if (!showHeatmap || !heatmapCtx || !map) return;

    heatmapCtx.clearRect(0, 0, heatmapCanvas.width, heatmapCanvas.height);

    currentHeatmapData.forEach(point => {
        const containerPoint = map.latLngToContainerPoint([point.lat, point.lng]);
        if (!containerPoint) return;

        const x = containerPoint.x;
        const y = containerPoint.y;
        const intensity = point.intensity || 0.5;
        const baseRadius = 40 + map.getZoom() * 3;
        const radius = baseRadius * (0.6 + intensity * 0.6);

        const gradient = heatmapCtx.createRadialGradient(x, y, 0, x, y, radius);
        const r = Math.round(intensity > 0.6 ? 239 : intensity > 0.3 ? 245 : 34);
        const g = Math.round(intensity > 0.6 ? 68 : intensity > 0.3 ? 158 : 197);
        const b = Math.round(intensity > 0.6 ? 68 : intensity > 0.3 ? 11 : 94);
        const alpha = intensity * 0.35;

        gradient.addColorStop(0, `rgba(${r}, ${g}, ${b}, ${alpha})`);
        gradient.addColorStop(0.5, `rgba(${r}, ${g}, ${b}, ${alpha * 0.4})`);
        gradient.addColorStop(1, `rgba(${r}, ${g}, ${b}, 0)`);

        heatmapCtx.fillStyle = gradient;
        heatmapCtx.beginPath();
        heatmapCtx.arc(x, y, radius, 0, Math.PI * 2);
        heatmapCtx.fill();
    });
}

function closeModal() {
    document.getElementById('detail-modal').classList.remove('active');
}

async function openDeviceDetail(deviceId) {
    const loc = allLocations.find(l => l.id === deviceId);
    if (!loc) return;

    const modal = document.getElementById('detail-modal');
    const body = document.getElementById('modal-body');
    const title = document.getElementById('modal-title');

    modal.classList.add('active');
    title.textContent = loc.name;
    body.innerHTML = '<div class="loading">正在加载数据...</div>';

    if (loc.device_type === 'soil_sensor') {
        body.innerHTML = renderSoilDetail(loc);
    } else {
        const [trendRes, predRes, stabRes] = await Promise.all([
            apiGet(`/corrosion/trend/${deviceId}?hours=168`),
            apiGet(`/corrosion/prediction/${deviceId}`),
            apiGet(`/corrosion/stability/${deviceId}`),
        ]);
        body.innerHTML = renderCorrosionDetail(
            loc,
            trendRes.success ? trendRes.data : [],
            predRes.success ? predRes.data : null,
            stabRes.success ? stabRes.data : null
        );
        setTimeout(() => renderTrendChart(trendRes.success ? trendRes.data : []), 50);
    }
}

function renderSoilDetail(loc) {
    return `
        <div class="detail-section">
            <h3>📋 基本信息</h3>
            <div class="info-grid">
                <div class="info-box">
                    <div class="info-box-label">设备ID</div>
                    <div class="info-box-value">${loc.id}</div>
                </div>
                <div class="info-box">
                    <div class="info-box-label">所在区域</div>
                    <div class="info-box-value">${loc.zone}</div>
                </div>
                <div class="info-box">
                    <div class="info-box-label">类型</div>
                    <div class="info-box-value">土壤环境传感器</div>
                </div>
                <div class="info-box">
                    <div class="info-box-label">坐标</div>
                    <div class="info-box-value">${loc.lat.toFixed(4)}, ${loc.lng.toFixed(4)}</div>
                </div>
            </div>
        </div>
        <div class="detail-section">
            <h3>📡 监测参数</h3>
            <div class="info-grid">
                <div class="info-box">
                    <div class="info-box-label">土壤温度</div>
                    <div class="info-box-value warning">-- °C</div>
                </div>
                <div class="info-box">
                    <div class="info-box-label">土壤湿度</div>
                    <div class="info-box-value">-- %</div>
                </div>
                <div class="info-box">
                    <div class="info-box-label">pH 值</div>
                    <div class="info-box-value">--</div>
                </div>
                <div class="info-box">
                    <div class="info-box-label">氯离子</div>
                    <div class="info-box-value">-- ppm</div>
                </div>
            </div>
        </div>
    `;
}

function renderCorrosionDetail(loc, trend, prediction, stability) {
    const matName = loc.material_type === 'iron' ? '铁器' : '铜器';
    const matColor = loc.material_type === 'iron' ? 'danger' : 'warning';

    let predHtml = '';
    if (prediction) {
        const riskClass = prediction.risk_level === '严重' ? 'critical'
            : prediction.risk_level === '高' ? 'high'
            : prediction.risk_level === '中等' ? 'medium' : 'low';
        predHtml = `
            <div class="detail-section">
                <h3>🔮 神经网络腐蚀预测</h3>
                <div class="info-grid" style="margin-bottom:14px;">
                    <div class="info-box">
                        <div class="info-box-label">当前腐蚀速率</div>
                        <div class="info-box-value ${prediction.current_rate > 0.5 ? 'danger' : 'warning'}">${prediction.current_rate.toFixed(4)} mm/y</div>
                    </div>
                    <div class="info-box">
                        <div class="info-box-label">风险等级</div>
                        <div class="info-box-value"><span class="risk-badge ${riskClass}">${prediction.risk_level}</span></div>
                    </div>
                    <div class="info-box">
                        <div class="info-box-label">预测置信度</div>
                        <div class="info-box-value success">${(prediction.confidence * 100).toFixed(0)}%</div>
                    </div>
                    <div class="info-box">
                        <div class="info-box-label">监测对象</div>
                        <div class="info-box-value ${matColor}">${matName}</div>
                    </div>
                </div>
                <div class="prediction-grid">
                    <div class="pred-box">
                        <div class="pred-label">7天预测</div>
                        <div class="pred-value">${prediction.predicted_rate_7d.toFixed(4)}</div>
                        <div class="pred-sub">mm/年</div>
                    </div>
                    <div class="pred-box">
                        <div class="pred-label">30天预测</div>
                        <div class="pred-value">${prediction.predicted_rate_30d.toFixed(4)}</div>
                        <div class="pred-sub">mm/年</div>
                    </div>
                    <div class="pred-box">
                        <div class="pred-label">90天预测</div>
                        <div class="pred-value">${prediction.predicted_rate_90d.toFixed(4)}</div>
                        <div class="pred-sub">mm/年</div>
                    </div>
                </div>
            </div>
        `;
    }

    let stabHtml = '';
    if (stability) {
        const stabLevelClass = stability.stability_level === '极稳定' || stability.stability_level === '稳定' ? 'low'
            : stability.stability_level === '较稳定' ? 'medium' : 'high';
        stabHtml = `
            <div class="detail-section">
                <h3>🛡️ 稳定性评估</h3>
                <div class="info-grid" style="margin-bottom:14px;">
                    <div class="info-box">
                        <div class="info-box-label">稳定指数</div>
                        <div class="info-box-value ${stability.stability_index > 0.7 ? 'success' : stability.stability_index > 0.4 ? 'warning' : 'danger'}">${(stability.stability_index * 100).toFixed(1)}%</div>
                    </div>
                    <div class="info-box">
                        <div class="info-box-label">稳定性等级</div>
                        <div class="info-box-value"><span class="risk-badge ${stabLevelClass}">${stability.stability_level}</span></div>
                    </div>
                    <div class="info-box">
                        <div class="info-box-label">预估剩余年限</div>
                        <div class="info-box-value">${stability.remaining_lifetime_years.toFixed(1)} 年</div>
                    </div>
                </div>
                <h3 style="margin-top:14px;">💡 保护建议</h3>
                <ul class="recommendations">
                    ${stability.recommendations.map(r => `<li>${r}</li>`).join('')}
                </ul>
            </div>
        `;
    }

    return `
        <div class="detail-section">
            <h3>📋 基本信息</h3>
            <div class="info-grid">
                <div class="info-box">
                    <div class="info-box-label">设备ID</div>
                    <div class="info-box-value">${loc.id}</div>
                </div>
                <div class="info-box">
                    <div class="info-box-label">所在区域</div>
                    <div class="info-box-value">${loc.zone}</div>
                </div>
                <div class="info-box">
                    <div class="info-box-label">监测材质</div>
                    <div class="info-box-value ${matColor}">${matName}</div>
                </div>
                <div class="info-box">
                    <div class="info-box-label">坐标</div>
                    <div class="info-box-value">${loc.lat.toFixed(4)}, ${loc.lng.toFixed(4)}</div>
                </div>
            </div>
        </div>

        <div class="detail-section">
            <h3>📈 腐蚀速率趋势 (近7天, 线性极化电阻法)</h3>
            <div class="chart-container">
                <canvas id="trend-chart" class="chart-canvas"></canvas>
            </div>
        </div>

        ${predHtml}
        ${stabHtml}
    `;
}

function renderTrendChart(data) {
    const canvas = document.getElementById('trend-chart');
    if (!canvas) return;

    const container = canvas.parentElement;
    canvas.width = container.clientWidth - 30;
    canvas.height = container.clientHeight - 30;

    const ctx = canvas.getContext('2d');
    const w = canvas.width;
    const h = canvas.height;
    const padding = { top: 20, right: 20, bottom: 30, left: 50 };
    const chartW = w - padding.left - padding.right;
    const chartH = h - padding.top - padding.bottom;

    ctx.clearRect(0, 0, w, h);

    if (!data || data.length === 0) {
        ctx.fillStyle = '#64748b';
        ctx.font = '13px sans-serif';
        ctx.textAlign = 'center';
        ctx.fillText('暂无数据', w / 2, h / 2);
        return;
    }

    const rates = data.map(d => d.corrosion_rate);
    const maxRate = Math.max(...rates, 0.6) * 1.1;
    const minRate = 0;

    ctx.strokeStyle = 'rgba(255,255,255,0.06)';
    ctx.lineWidth = 1;
    for (let i = 0; i <= 4; i++) {
        const y = padding.top + (chartH / 4) * i;
        ctx.beginPath();
        ctx.moveTo(padding.left, y);
        ctx.lineTo(w - padding.right, y);
        ctx.stroke();

        const val = maxRate - (maxRate - minRate) * (i / 4);
        ctx.fillStyle = '#64748b';
        ctx.font = '10px sans-serif';
        ctx.textAlign = 'right';
        ctx.fillText(val.toFixed(2) + ' mm/y', padding.left - 6, y + 3);
    }

    const threshold = 0.5;
    const thresholdY = padding.top + chartH - ((threshold - minRate) / (maxRate - minRate)) * chartH;
    if (thresholdY > padding.top && thresholdY < padding.top + chartH) {
        ctx.strokeStyle = 'rgba(239, 68, 68, 0.5)';
        ctx.setLineDash([5, 4]);
        ctx.lineWidth = 1.5;
        ctx.beginPath();
        ctx.moveTo(padding.left, thresholdY);
        ctx.lineTo(w - padding.right, thresholdY);
        ctx.stroke();
        ctx.setLineDash([]);

        ctx.fillStyle = 'rgba(239, 68, 68, 0.7)';
        ctx.font = '10px sans-serif';
        ctx.textAlign = 'left';
        ctx.fillText('告警阈值 0.5 mm/y', padding.left + 4, thresholdY - 4);
    }

    const gradient = ctx.createLinearGradient(0, padding.top, 0, padding.top + chartH);
    gradient.addColorStop(0, 'rgba(251, 191, 36, 0.8)');
    gradient.addColorStop(0.5, 'rgba(245, 158, 11, 0.6)');
    gradient.addColorStop(1, 'rgba(239, 68, 68, 0.4)');

    ctx.strokeStyle = gradient;
    ctx.lineWidth = 2.5;
    ctx.lineJoin = 'round';
    ctx.lineCap = 'round';
    ctx.beginPath();

    data.forEach((d, i) => {
        const x = padding.left + (chartW / (data.length - 1 || 1)) * i;
        const y = padding.top + chartH - ((d.corrosion_rate - minRate) / (maxRate - minRate)) * chartH;
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
    });
    ctx.stroke();

    const lastX = padding.left + chartW;
    const firstX = padding.left;
    ctx.lineTo(lastX, padding.top + chartH);
    ctx.lineTo(firstX, padding.top + chartH);
    ctx.closePath();

    const areaGrad = ctx.createLinearGradient(0, padding.top, 0, padding.top + chartH);
    areaGrad.addColorStop(0, 'rgba(251, 191, 36, 0.25)');
    areaGrad.addColorStop(1, 'rgba(251, 191, 36, 0.02)');
    ctx.fillStyle = areaGrad;
    ctx.fill();

    data.forEach((d, i) => {
        const x = padding.left + (chartW / (data.length - 1 || 1)) * i;
        const y = padding.top + chartH - ((d.corrosion_rate - minRate) / (maxRate - minRate)) * chartH;

        if (i % Math.max(1, Math.floor(data.length / 8)) === 0 || i === data.length - 1) {
            ctx.beginPath();
            ctx.arc(x, y, 3.5, 0, Math.PI * 2);
            ctx.fillStyle = d.corrosion_rate > 0.5 ? '#ef4444' : '#fbbf24';
            ctx.fill();
            ctx.strokeStyle = '#1e293b';
            ctx.lineWidth = 1.5;
            ctx.stroke();
        }
    });

    if (data.length > 0) {
        const firstTs = new Date(data[0].timestamp);
        const lastTs = new Date(data[data.length - 1].timestamp);
        ctx.fillStyle = '#64748b';
        ctx.font = '10px sans-serif';
        ctx.textAlign = 'left';
        ctx.fillText(formatDate(firstTs), padding.left, h - 8);
        ctx.textAlign = 'right';
        ctx.fillText(formatDate(lastTs), w - padding.right, h - 8);
    }
}

function formatDate(d) {
    const month = d.getMonth() + 1;
    const day = d.getDate();
    const hours = d.getHours().toString().padStart(2, '0');
    return `${month}/${day} ${hours}:00`;
}

window.openDeviceDetail = openDeviceDetail;
document.addEventListener('DOMContentLoaded', init);
