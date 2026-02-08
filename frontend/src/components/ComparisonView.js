import React, { useState, useCallback } from 'react';
import { AreaChart, Area, XAxis, YAxis, Tooltip, ResponsiveContainer, ReferenceLine } from 'recharts';
import { Play, Loader } from 'lucide-react';
import api from '../services/api';
import './ComparisonView.css';

function ComparisonView({ currentMetrics, uploadedFilePath, uploadedFileName }) {
  const [comparisonData, setComparisonData] = useState(null);
  const [loading, setLoading] = useState(false);
  const [fileInfo, setFileInfo] = useState(null);

  const lossRate = currentMetrics ? currentMetrics.loss_rate * 100 : 0;
  const hasFile = !!uploadedFilePath;

  const handleRunComparison = useCallback(async () => {
    if (!uploadedFilePath) return;
    setLoading(true);
    try {
      const result = await api.simulateComparison(uploadedFilePath);
      setFileInfo({
        fileName: result.file_name,
        fileSize: result.file_size_bytes,
        totalChunks: result.total_chunks,
        dataChunks: result.data_chunks,
        parityChunks: result.parity_chunks,
        trialsPerPoint: result.trials_per_point,
      });
      const points = result.points.map(p => ({
        loss: `${p.loss_percent}%`,
        lossNum: p.loss_percent,
        tcpSuccess: Math.round(p.tcp_success_rate * 10) / 10,
        resilientSuccess: Math.round(p.resilient_success_rate * 10) / 10,
      }));
      setComparisonData(points);
    } catch (err) {
      console.error('Comparison simulation failed:', err);
    }
    setLoading(false);
  }, [uploadedFilePath]);

  const currentIndex = comparisonData
    ? Math.min(Math.round(lossRate), comparisonData.length - 1)
    : 0;
  const currentPoint = comparisonData
    ? (comparisonData[currentIndex] || comparisonData[0])
    : null;

  return (
    <div className="comparison">
      <div className="section-header">
        <h3>TCP vs RESILIENT Comparison</h3>
        <span className="comparison-subtitle">
          {comparisonData && fileInfo
            ? `${fileInfo.fileName} -- ${fileInfo.trialsPerPoint} trials/point -- ${fileInfo.totalChunks} chunks (${fileInfo.dataChunks}D+${fileInfo.parityChunks}P)`
            : 'Run comparison on your uploaded file'}
        </span>
      </div>

      {/* Run button */}
      {!comparisonData && (
        <div className="comparison-run-section">
          <button
            className="comparison-run-btn"
            onClick={handleRunComparison}
            disabled={loading || !hasFile}
          >
            {loading ? (
              <><Loader size={14} className="spin" /> Running 41 loss rates x 20 trials...</>
            ) : (
              <><Play size={14} /> Run Comparison{hasFile ? ` on ${uploadedFileName}` : ''}</>
            )}
          </button>
          {!hasFile && (
            <span className="comparison-hint">Upload a file first</span>
          )}
        </div>
      )}

      {/* Results */}
      {comparisonData && currentPoint && (
        <>
          {/* Summary cards */}
          <div className="comparison-cards">
            <div className="comp-card tcp">
              <div className="comp-card-header">
                <span className="comp-card-title">Without RESILIENT (TCP)</span>
              </div>
              <div className="comp-card-stats">
                <div className="comp-stat">
                  <span className="comp-stat-value mono">{currentPoint.tcpSuccess}%</span>
                  <span className="comp-stat-label">Recovery Rate</span>
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
                  <span className="comp-stat-label">Recovery Rate</span>
                </div>
              </div>
              {currentPoint.resilientSuccess >= 100 && (
                <div className="comp-success">Full recovery -- zero data loss</div>
              )}
              {currentPoint.resilientSuccess === 0 && lossRate > 0 && (
                <div className="comp-warning">Beyond recovery threshold</div>
              )}
            </div>
          </div>

          {/* Chart */}
          <div className="comparison-charts">
            <div className="chart-section" style={{ gridColumn: '1 / -1' }}>
              <span className="chart-title">File Recovery Rate vs Packet Loss (real simulation)</span>
              <div className="chart-container">
                <ResponsiveContainer width="100%" height={220}>
                  <AreaChart data={comparisonData} margin={{ top: 5, right: 5, left: -20, bottom: 0 }}>
                    <defs>
                      <linearGradient id="tcpFill" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="0%" stopColor="#dc2626" stopOpacity={0.15} />
                        <stop offset="100%" stopColor="#dc2626" stopOpacity={0} />
                      </linearGradient>
                      <linearGradient id="resilientFill" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="0%" stopColor="#111827" stopOpacity={0.15} />
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
          </div>

          {/* Re-run button */}
          <div className="comparison-rerun">
            <button
              className="comparison-rerun-btn"
              onClick={handleRunComparison}
              disabled={loading}
            >
              {loading ? (
                <><Loader size={12} className="spin" /> Running...</>
              ) : (
                <><Play size={12} /> Re-run Comparison</>
              )}
            </button>
          </div>
        </>
      )}
    </div>
  );
}

export default ComparisonView;
