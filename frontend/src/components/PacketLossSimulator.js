import React, { useState, useCallback } from 'react';
import { Sliders, Play, RotateCcw, FileText } from 'lucide-react';
import api from '../services/api';
import './PacketLossSimulator.css';

function PacketLossSimulator({ currentMetrics, uploadedFilePath, uploadedFileName }) {
  const [lossRate, setLossRate] = useState(0);
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState(null);

  const hasFile = !!uploadedFilePath;

  const handleSimulate = useCallback(async () => {
    setRunning(true);
    try {
      const res = await api.simulatePacketLoss(lossRate / 100, uploadedFilePath);
      setResult(res);
    } catch (err) {
      console.error('Simulation failed:', err);
    }
    setRunning(false);
  }, [lossRate, uploadedFilePath]);

  const handleReset = useCallback(async () => {
    setLossRate(0);
    setResult(null);
    try {
      await api.simulatePacketLoss(0);
    } catch (err) {
      // ignore
    }
  }, []);

  const getZoneLabel = (rate) => {
    if (rate <= 5) return 'Normal';
    if (rate <= 10) return 'Mild Loss';
    if (rate <= 20) return 'Degraded';
    if (rate <= 33) return 'Severe';
    return 'Critical';
  };

  const getZoneColor = (rate) => {
    if (rate <= 5) return 'var(--success)';
    if (rate <= 10) return 'var(--text-primary)';
    if (rate <= 20) return 'var(--warning)';
    return 'var(--danger)';
  };

  const formatBytes = (bytes) => {
    if (!bytes) return '0 B';
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  const getSuccessBannerClass = (rate) => {
    if (rate >= 100) return 'success';
    if (rate >= 70) return 'warning';
    return 'failure';
  };

  const getSuccessBannerText = (result) => {
    const rate = result.success_rate;
    if (rate >= 100) return `ALL ${result.num_trials} TRIALS RECOVERED -- 100% success rate`;
    if (rate > 0) return `${result.successful_trials}/${result.num_trials} TRIALS RECOVERED -- ${rate.toFixed(0)}% success rate`;
    return `ALL ${result.num_trials} TRIALS FAILED -- 0% recovery`;
  };

  return (
    <div className="packet-sim">
      <div className="section-header">
        <div className="sim-header-left">
          <Sliders size={16} />
          <h3>Packet Loss Simulator</h3>
        </div>
        <span className="sim-zone" style={{ color: getZoneColor(lossRate) }}>
          {getZoneLabel(lossRate)}
        </span>
      </div>

      <div className="sim-body">
        {/* File indicator */}
        <div className="sim-file-indicator">
          <FileText size={14} />
          {hasFile ? (
            <span className="sim-file-name">
              Simulating on: <strong>{uploadedFileName || 'uploaded file'}</strong>
            </span>
          ) : (
            <span className="sim-file-missing">
              Upload a file first to run simulation
            </span>
          )}
        </div>

        <div className="slider-section">
          <div className="slider-header">
            <span className="slider-label">Simulated Loss Rate</span>
            <span className="slider-value mono">{lossRate}%</span>
          </div>

          <div className="slider-track-wrapper">
            <input
              type="range"
              min="0"
              max="50"
              step="1"
              value={lossRate}
              onChange={(e) => setLossRate(parseInt(e.target.value))}
              className="loss-slider"
              disabled={!hasFile}
              style={{
                background: `linear-gradient(to right, var(--accent) 0%, var(--accent) ${lossRate * 2}%, var(--border) ${lossRate * 2}%, var(--border) 100%)`
              }}
            />

            {/* Scale markers */}
            <div className="slider-scale">
              <span className="scale-marker" style={{ left: '0%' }}>0%</span>
              <span className="scale-marker" style={{ left: '16%' }}>8%</span>
              <span className="scale-marker" style={{ left: '40%' }}>20%</span>
              <span className="scale-marker danger" style={{ left: '66%' }}>33%</span>
              <span className="scale-marker" style={{ left: '100%' }}>50%</span>
            </div>
          </div>
        </div>

        <div className="sim-actions">
          <button
            className="sim-btn primary"
            onClick={handleSimulate}
            disabled={running || !hasFile}
            title={!hasFile ? 'Upload a file first' : ''}
          >
            <Play size={14} />
            {running ? 'Running...' : 'Simulate'}
          </button>
          <button
            className="sim-btn secondary"
            onClick={handleReset}
          >
            <RotateCcw size={14} />
            Reset
          </button>
        </div>

        {result && (
          <div className="sim-result">
            {/* Trial success banner */}
            {result.file_name && result.success_rate != null && (
              <div className={`sim-recovery-banner ${getSuccessBannerClass(result.success_rate)}`}>
                {getSuccessBannerText(result)}
              </div>
            )}

            <div className="result-grid">
              <div className="result-item">
                <span className="result-label">Applied Loss</span>
                <span className="result-value mono">{(result.applied_loss_rate * 100).toFixed(0)}%</span>
              </div>
              <div className="result-item">
                <span className="result-label">Parity Response</span>
                <span className="result-value mono">{result.resulting_parity} shards</span>
              </div>
              <div className="result-item">
                <span className="result-label">Recovery Cap.</span>
                <span className="result-value mono">{result.recovery_capability.toFixed(1)}%</span>
              </div>
              <div className="result-item">
                <span className="result-label">Overhead</span>
                <span className="result-value mono">{result.overhead_percent.toFixed(1)}%</span>
              </div>
            </div>

            {/* Detailed file simulation results */}
            {result.file_name && (
              <div className="result-grid file-result-grid">
                <div className="result-item">
                  <span className="result-label">File</span>
                  <span className="result-value mono">{result.file_name}</span>
                </div>
                <div className="result-item">
                  <span className="result-label">File Size</span>
                  <span className="result-value mono">{formatBytes(result.file_size_bytes)}</span>
                </div>
                <div className="result-item">
                  <span className="result-label">Chunks</span>
                  <span className="result-value mono">{result.total_chunks} ({result.data_chunks}D + {result.parity_chunks}P)</span>
                </div>
                <div className="result-item">
                  <span className="result-label">Trials Run</span>
                  <span className="result-value mono">{result.num_trials}</span>
                </div>
                <div className="result-item">
                  <span className="result-label">Success Rate</span>
                  <span className={`result-value mono ${result.success_rate >= 100 ? 'success' : result.success_rate > 0 ? 'warning-text' : 'danger'}`}>
                    {result.success_rate.toFixed(0)}%
                  </span>
                </div>
                <div className="result-item">
                  <span className="result-label">Avg Lost</span>
                  <span className="result-value mono">{result.avg_chunks_lost.toFixed(1)}</span>
                </div>
                <div className="result-item">
                  <span className="result-label">Avg Recovered</span>
                  <span className="result-value mono success">{result.avg_chunks_recovered.toFixed(1)}</span>
                </div>
                <div className="result-item">
                  <span className="result-label">Lost Range</span>
                  <span className="result-value mono">{result.min_chunks_lost} - {result.max_chunks_lost}</span>
                </div>
                <div className="result-item">
                  <span className="result-label">Recovered</span>
                  <span className={`result-value mono ${result.success_rate >= 100 ? 'success' : 'danger'}`}>
                    {result.successful_trials}/{result.num_trials}
                  </span>
                </div>
              </div>
            )}

            <div className="result-message">{result.message}</div>
          </div>
        )}
      </div>
    </div>
  );
}

export default PacketLossSimulator;
