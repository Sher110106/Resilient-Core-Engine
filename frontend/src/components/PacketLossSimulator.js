import React, { useState, useCallback } from 'react';
import { Sliders, Play, RotateCcw } from 'lucide-react';
import api from '../services/api';
import './PacketLossSimulator.css';

function PacketLossSimulator({ currentMetrics }) {
  const [lossRate, setLossRate] = useState(0);
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState(null);

  const handleSimulate = useCallback(async () => {
    setRunning(true);
    try {
      const res = await api.simulatePacketLoss(lossRate / 100);
      setResult(res);
    } catch (err) {
      console.error('Simulation failed:', err);
    }
    setRunning(false);
  }, [lossRate]);

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
            disabled={running}
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
                <span className="result-label">Recovery</span>
                <span className="result-value mono">{result.recovery_capability.toFixed(1)}%</span>
              </div>
              <div className="result-item">
                <span className="result-label">Overhead</span>
                <span className="result-value mono">{result.overhead_percent.toFixed(1)}%</span>
              </div>
            </div>
            <div className="result-message">{result.message}</div>
          </div>
        )}
      </div>
    </div>
  );
}

export default PacketLossSimulator;
