import React from 'react';
import { AreaChart, Area, LineChart, Line, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts';
import './MetricsPanel.css';

function MetricsPanel({ metricsHistory, currentMetrics }) {
  const lossData = metricsHistory.map((m, i) => ({
    time: m.time || i,
    loss: (m.loss_rate * 100),
    recovery: m.recovery_capability,
    parity: m.parity_shards,
    rtt: m.quic_rtt_ms || 0,
  }));

  const parity = currentMetrics ? currentMetrics.parity_shards : 5;
  const dataShards = currentMetrics ? currentMetrics.data_shards : 50;
  const overhead = currentMetrics ? currentMetrics.overhead_percent : 0;

  // Real QUIC stats
  const quicRtt = currentMetrics ? (currentMetrics.quic_rtt_ms || 0) : 0;
  const quicLossRate = currentMetrics ? (currentMetrics.quic_loss_rate || 0) : 0;
  const quicSent = currentMetrics ? (currentMetrics.quic_sent_packets || 0) : 0;
  const quicLost = currentMetrics ? (currentMetrics.quic_lost_packets || 0) : 0;
  const hasQuicData = quicSent > 0;

  // Gauge calculation for loss rate
  const lossRate = currentMetrics ? currentMetrics.loss_rate * 100 : 0;
  const maxLoss = 50;
  const gaugePercent = Math.min(lossRate / maxLoss, 1);
  const circumference = 2 * Math.PI * 40;
  const dashOffset = circumference * (1 - gaugePercent * 0.75);

  return (
    <div className="metrics-panel">
      <div className="section-header">
        <h3>Live Metrics</h3>
      </div>

      <div className="metrics-grid">
        {/* Loss Rate Gauge */}
        <div className="metric-gauge-container">
          <svg viewBox="0 0 100 80" className="gauge-svg">
            {/* Background arc */}
            <circle
              cx="50" cy="50" r="40"
              fill="none"
              stroke="#e5e7eb"
              strokeWidth="6"
              strokeDasharray={circumference}
              strokeDashoffset={circumference * 0.25}
              strokeLinecap="round"
              transform="rotate(135 50 50)"
            />
            {/* Value arc */}
            <circle
              cx="50" cy="50" r="40"
              fill="none"
              stroke={lossRate > 20 ? '#dc2626' : lossRate > 10 ? '#d97706' : '#111827'}
              strokeWidth="6"
              strokeDasharray={circumference}
              strokeDashoffset={dashOffset}
              strokeLinecap="round"
              transform="rotate(135 50 50)"
              className="gauge-arc"
            />
            <text x="50" y="48" textAnchor="middle" className="gauge-value">
              {lossRate.toFixed(1)}%
            </text>
            <text x="50" y="60" textAnchor="middle" className="gauge-label">
              LOSS RATE
            </text>
          </svg>
        </div>

        {/* Erasure Config */}
        <div className="metric-config">
          <div className="config-row">
            <span className="config-label">Data Shards</span>
            <span className="config-value mono">{dataShards}</span>
          </div>
          <div className="config-row">
            <span className="config-label">Parity Shards</span>
            <span className="config-value mono">{parity}</span>
          </div>
          <div className="config-row">
            <span className="config-label">Overhead</span>
            <span className="config-value mono">{overhead.toFixed(1)}%</span>
          </div>
          <div className="config-row">
            <span className="config-label">Total</span>
            <span className="config-value mono">{dataShards + parity} shards</span>
          </div>
        </div>
      </div>

      {/* Real QUIC Network Stats */}
      {hasQuicData && (
        <div className="quic-stats-section">
          <span className="chart-title">Real QUIC Network Stats</span>
          <div className="quic-stats-grid">
            <div className="quic-stat">
              <span className="quic-stat-value mono">{quicRtt.toFixed(1)}ms</span>
              <span className="quic-stat-label">RTT</span>
            </div>
            <div className="quic-stat">
              <span className={`quic-stat-value mono ${quicLossRate > 0.05 ? 'danger' : ''}`}>
                {(quicLossRate * 100).toFixed(2)}%
              </span>
              <span className="quic-stat-label">Packet Loss</span>
            </div>
            <div className="quic-stat">
              <span className="quic-stat-value mono">{quicSent}</span>
              <span className="quic-stat-label">Packets Sent</span>
            </div>
            <div className="quic-stat">
              <span className={`quic-stat-value mono ${quicLost > 0 ? 'warning-text' : ''}`}>
                {quicLost}
              </span>
              <span className="quic-stat-label">Packets Lost</span>
            </div>
          </div>
        </div>
      )}

      {/* Loss Rate / Recovery Timeline */}
      {lossData.length > 2 && (
        <div className="chart-section">
          <span className="chart-title">Packet Loss & Recovery Capability</span>
          <div className="chart-container">
            <ResponsiveContainer width="100%" height={140}>
              <AreaChart data={lossData} margin={{ top: 5, right: 5, left: -20, bottom: 0 }}>
                <defs>
                  <linearGradient id="lossGrad" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stopColor="#111827" stopOpacity={0.15} />
                    <stop offset="100%" stopColor="#111827" stopOpacity={0} />
                  </linearGradient>
                  <linearGradient id="recoveryGrad" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stopColor="#059669" stopOpacity={0.15} />
                    <stop offset="100%" stopColor="#059669" stopOpacity={0} />
                  </linearGradient>
                </defs>
                <XAxis dataKey="time" tick={false} axisLine={false} />
                <YAxis tick={{ fontSize: 10, fill: '#9ca3af' }} axisLine={false} tickLine={false} />
                <Tooltip
                  contentStyle={{
                    background: '#fff',
                    border: '1px solid #e5e7eb',
                    borderRadius: '6px',
                    fontSize: '12px',
                    boxShadow: '0 4px 6px rgba(0,0,0,0.05)'
                  }}
                  formatter={(value, name) => [
                    `${value.toFixed(1)}%`,
                    name === 'loss' ? 'Packet Loss' : 'Recovery'
                  ]}
                />
                <Area
                  type="monotone"
                  dataKey="loss"
                  stroke="#111827"
                  fill="url(#lossGrad)"
                  strokeWidth={1.5}
                  dot={false}
                />
                <Area
                  type="monotone"
                  dataKey="recovery"
                  stroke="#059669"
                  fill="url(#recoveryGrad)"
                  strokeWidth={1.5}
                  dot={false}
                />
              </AreaChart>
            </ResponsiveContainer>
          </div>
        </div>
      )}

      {/* Parity Adaptation */}
      {lossData.length > 2 && (
        <div className="chart-section">
          <span className="chart-title">Parity Shard Adaptation</span>
          <div className="chart-container">
            <ResponsiveContainer width="100%" height={100}>
              <LineChart data={lossData} margin={{ top: 5, right: 5, left: -20, bottom: 0 }}>
                <XAxis dataKey="time" tick={false} axisLine={false} />
                <YAxis
                  tick={{ fontSize: 10, fill: '#9ca3af' }}
                  axisLine={false}
                  tickLine={false}
                  domain={[0, 30]}
                />
                <Tooltip
                  contentStyle={{
                    background: '#fff',
                    border: '1px solid #e5e7eb',
                    borderRadius: '6px',
                    fontSize: '12px'
                  }}
                  formatter={(value) => [`${value} shards`, 'Parity']}
                />
                <Line
                  type="stepAfter"
                  dataKey="parity"
                  stroke="#111827"
                  strokeWidth={2}
                  dot={false}
                />
              </LineChart>
            </ResponsiveContainer>
          </div>
        </div>
      )}
    </div>
  );
}

export default MetricsPanel;
