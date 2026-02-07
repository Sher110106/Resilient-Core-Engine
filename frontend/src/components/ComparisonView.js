import React, { useMemo } from 'react';
import { AreaChart, Area, XAxis, YAxis, Tooltip, ResponsiveContainer, ReferenceLine } from 'recharts';
import './ComparisonView.css';

function ComparisonView({ currentMetrics }) {
  const lossRate = currentMetrics ? currentMetrics.loss_rate * 100 : 0;

  // Generate comparison data: TCP vs RESILIENT across loss rates
  const comparisonData = useMemo(() => {
    const points = [];
    for (let loss = 0; loss <= 40; loss += 1) {
      // TCP: degrades rapidly after 1-2% loss
      // Real TCP retransmits, but throughput collapses and failure rate rises sharply
      const tcpSuccess = loss <= 1 ? 100 : loss <= 3 ? 100 - (loss - 1) * 15 : Math.max(0, 70 - (loss - 3) * 8);
      const tcpThroughput = loss <= 1 ? 100 : loss <= 5 ? 100 - loss * 12 : Math.max(0, 40 - (loss - 5) * 5);

      // RESILIENT: Reed-Solomon is all-or-nothing.
      // If loss <= max recoverable for current parity level -> 100% integrity.
      // If loss > max recoverable -> 0% (cannot reconstruct at all).
      let parity;
      if (loss <= 5) parity = 5;
      else if (loss <= 10) parity = 10;
      else if (loss <= 15) parity = 15;
      else if (loss <= 20) parity = 20;
      else parity = 25;

      const maxRecoverable = (parity / (50 + parity)) * 100;
      const resilientSuccess = loss <= maxRecoverable ? 100 : 0;
      const overhead = (parity / (50 + parity)) * 100;
      const resilientThroughput = loss <= maxRecoverable
        ? Math.max(20, 100 - overhead - loss * 0.3)
        : 0;

      points.push({
        loss: `${loss}%`,
        lossNum: loss,
        tcpSuccess: Math.max(0, Math.round(tcpSuccess * 10) / 10),
        resilientSuccess: Math.max(0, Math.round(resilientSuccess * 10) / 10),
        tcpThroughput: Math.max(0, Math.round(tcpThroughput * 10) / 10),
        resilientThroughput: Math.max(0, Math.round(resilientThroughput * 10) / 10),
      });
    }
    return points;
  }, []);

  const currentIndex = Math.min(Math.round(lossRate), 40);
  const currentPoint = comparisonData[currentIndex] || comparisonData[0];

  return (
    <div className="comparison">
      <div className="section-header">
        <h3>TCP vs RESILIENT Comparison</h3>
        <span className="comparison-subtitle">Side-by-side at {lossRate.toFixed(1)}% packet loss</span>
      </div>

      {/* Summary cards */}
      <div className="comparison-cards">
        <div className="comp-card tcp">
          <div className="comp-card-header">
            <span className="comp-card-title">Without RESILIENT (TCP)</span>
          </div>
          <div className="comp-card-stats">
            <div className="comp-stat">
              <span className="comp-stat-value mono">{currentPoint.tcpSuccess}%</span>
              <span className="comp-stat-label">Data Integrity</span>
            </div>
            <div className="comp-stat">
              <span className="comp-stat-value mono">{currentPoint.tcpThroughput}%</span>
              <span className="comp-stat-label">Effective Throughput</span>
            </div>
          </div>
          {currentPoint.tcpSuccess < 50 && (
            <div className="comp-warning">Transfer likely fails at this loss rate</div>
          )}
        </div>

        <div className="comp-card resilient">
          <div className="comp-card-header">
            <span className="comp-card-title">With RESILIENT</span>
          </div>
          <div className="comp-card-stats">
            <div className="comp-stat">
              <span className="comp-stat-value mono">{currentPoint.resilientSuccess}%</span>
              <span className="comp-stat-label">Data Integrity</span>
            </div>
            <div className="comp-stat">
              <span className="comp-stat-value mono">{currentPoint.resilientThroughput}%</span>
              <span className="comp-stat-label">Effective Throughput</span>
            </div>
          </div>
          {currentPoint.resilientSuccess === 100 && (
            <div className="comp-success">Full recovery -- zero data loss</div>
          )}
          {currentPoint.resilientSuccess === 0 && lossRate > 0 && (
            <div className="comp-warning">Beyond recovery threshold -- transfer fails</div>
          )}
        </div>
      </div>

      {/* Charts */}
      <div className="comparison-charts">
        <div className="chart-section">
          <span className="chart-title">Data Integrity vs Packet Loss</span>
          <div className="chart-container">
            <ResponsiveContainer width="100%" height={180}>
              <AreaChart data={comparisonData} margin={{ top: 5, right: 5, left: -20, bottom: 0 }}>
                <defs>
                  <linearGradient id="tcpFill" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stopColor="#dc2626" stopOpacity={0.1} />
                    <stop offset="100%" stopColor="#dc2626" stopOpacity={0} />
                  </linearGradient>
                  <linearGradient id="resilientFill" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stopColor="#111827" stopOpacity={0.1} />
                    <stop offset="100%" stopColor="#111827" stopOpacity={0} />
                  </linearGradient>
                </defs>
                <XAxis
                  dataKey="loss"
                  tick={{ fontSize: 9, fill: '#9ca3af' }}
                  axisLine={{ stroke: '#e5e7eb' }}
                  tickLine={false}
                  interval={9}
                />
                <YAxis
                  tick={{ fontSize: 9, fill: '#9ca3af' }}
                  axisLine={false}
                  tickLine={false}
                  domain={[0, 105]}
                  tickFormatter={(v) => `${v}%`}
                />
                <Tooltip
                  contentStyle={{
                    background: '#fff',
                    border: '1px solid #e5e7eb',
                    borderRadius: '6px',
                    fontSize: '11px',
                    boxShadow: '0 4px 6px rgba(0,0,0,0.05)'
                  }}
                  formatter={(value, name) => [
                    `${value}%`,
                    name === 'tcpSuccess' ? 'TCP' : 'RESILIENT'
                  ]}
                  labelFormatter={(label) => `Packet Loss: ${label}`}
                />
                {lossRate > 0 && (
                  <ReferenceLine
                    x={`${Math.round(lossRate)}%`}
                    stroke="#d97706"
                    strokeDasharray="3 3"
                    strokeWidth={1}
                  />
                )}
                <Area
                  type="monotone"
                  dataKey="tcpSuccess"
                  stroke="#dc2626"
                  fill="url(#tcpFill)"
                  strokeWidth={1.5}
                  dot={false}
                  name="tcpSuccess"
                />
                <Area
                  type="monotone"
                  dataKey="resilientSuccess"
                  stroke="#111827"
                  fill="url(#resilientFill)"
                  strokeWidth={2}
                  dot={false}
                  name="resilientSuccess"
                />
              </AreaChart>
            </ResponsiveContainer>
          </div>
          <div className="chart-legend">
            <span className="legend-item">
              <span className="legend-line" style={{ background: '#111827' }} />
              RESILIENT
            </span>
            <span className="legend-item">
              <span className="legend-line" style={{ background: '#dc2626' }} />
              TCP
            </span>
            {lossRate > 0 && (
              <span className="legend-item">
                <span className="legend-line dashed" style={{ background: '#d97706' }} />
                Current
              </span>
            )}
          </div>
        </div>

        <div className="chart-section">
          <span className="chart-title">Effective Throughput vs Packet Loss</span>
          <div className="chart-container">
            <ResponsiveContainer width="100%" height={180}>
              <AreaChart data={comparisonData} margin={{ top: 5, right: 5, left: -20, bottom: 0 }}>
                <XAxis
                  dataKey="loss"
                  tick={{ fontSize: 9, fill: '#9ca3af' }}
                  axisLine={{ stroke: '#e5e7eb' }}
                  tickLine={false}
                  interval={9}
                />
                <YAxis
                  tick={{ fontSize: 9, fill: '#9ca3af' }}
                  axisLine={false}
                  tickLine={false}
                  domain={[0, 105]}
                  tickFormatter={(v) => `${v}%`}
                />
                <Tooltip
                  contentStyle={{
                    background: '#fff',
                    border: '1px solid #e5e7eb',
                    borderRadius: '6px',
                    fontSize: '11px'
                  }}
                  formatter={(value, name) => [
                    `${value}%`,
                    name === 'tcpThroughput' ? 'TCP' : 'RESILIENT'
                  ]}
                  labelFormatter={(label) => `Packet Loss: ${label}`}
                />
                {lossRate > 0 && (
                  <ReferenceLine
                    x={`${Math.round(lossRate)}%`}
                    stroke="#d97706"
                    strokeDasharray="3 3"
                    strokeWidth={1}
                  />
                )}
                <Area
                  type="monotone"
                  dataKey="tcpThroughput"
                  stroke="#dc2626"
                  fill="url(#tcpFill)"
                  strokeWidth={1.5}
                  dot={false}
                  name="tcpThroughput"
                />
                <Area
                  type="monotone"
                  dataKey="resilientThroughput"
                  stroke="#111827"
                  fill="url(#resilientFill)"
                  strokeWidth={2}
                  dot={false}
                  name="resilientThroughput"
                />
              </AreaChart>
            </ResponsiveContainer>
          </div>
        </div>
      </div>
    </div>
  );
}

export default ComparisonView;
