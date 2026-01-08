import React from 'react';
import './TransferList.css';

// Status Icons
const PlayIcon = () => (
  <svg className="action-icon" viewBox="0 0 24 24" fill="currentColor">
    <polygon points="5 3 19 12 5 21 5 3"/>
  </svg>
);

const PauseIcon = () => (
  <svg className="action-icon" viewBox="0 0 24 24" fill="currentColor">
    <rect x="6" y="4" width="4" height="16"/>
    <rect x="14" y="4" width="4" height="16"/>
  </svg>
);

const CancelIcon = () => (
  <svg className="action-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <line x1="18" y1="6" x2="6" y2="18"/>
    <line x1="6" y1="6" x2="18" y2="18"/>
  </svg>
);

const SignalIcon = () => (
  <svg className="status-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <path d="M5 12.55a11 11 0 0 1 14.08 0"/>
    <path d="M8.53 16.11a6 6 0 0 1 6.95 0"/>
    <circle cx="12" cy="20" r="1" fill="currentColor"/>
  </svg>
);

const PackageIcon = () => (
  <svg className="empty-icon-svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
    <path d="M16.5 9.4l-9-5.19"/>
    <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/>
    <polyline points="3.27 6.96 12 12.01 20.73 6.96"/>
    <line x1="12" y1="22.08" x2="12" y2="12"/>
  </svg>
);

function TransferList({ transfers, onPause, onResume, onCancel }) {
  const formatBytes = (bytes) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return Math.round(bytes / Math.pow(k, i) * 100) / 100 + ' ' + sizes[i];
  };

  const getStatusColor = (status) => {
    if (status === 'Active') return 'status-active';
    if (status === 'Completed') return 'status-completed';
    if (status === 'Paused') return 'status-paused';
    if (status?.Failed) return 'status-failed';
    return 'status-default';
  };

  const getStatusText = (status) => {
    if (typeof status === 'object' && status.Failed) {
      return 'Transmission Failed';
    }
    if (status === 'Active') return 'Transmitting...';
    if (status === 'Completed') return 'Intel Delivered';
    if (status === 'Paused') return 'On Hold';
    return status || 'Unknown';
  };

  const getStatusMessage = (transfer) => {
    if (transfer.status === 'Active') {
      if (transfer.progress_percent < 30) {
        return 'Establishing secure channel...';
      } else if (transfer.progress_percent < 70) {
        return 'Erasure coding active — recovering lost packets';
      } else {
        return 'Finalizing transmission...';
      }
    }
    return null;
  };

  const canPause = (status) => status === 'Active';
  const canResume = (status) => status === 'Paused';

  if (!transfers || transfers.length === 0) {
    return (
      <div className="transfer-list-container">
        <h2>
          <SignalIcon />
          Active Missions
        </h2>
        <div className="empty-state">
          <PackageIcon />
          <p>No active missions</p>
          <p className="empty-hint">Queue critical data for transmission above</p>
        </div>
      </div>
    );
  }

  return (
    <div className="transfer-list-container">
      <h2>
        <SignalIcon />
        Active Missions ({transfers.length})
      </h2>
      
      <div className="transfer-list">
        {transfers.map((transfer) => (
          <div key={transfer.session_id} className={`transfer-item ${getStatusColor(transfer.status)}`}>
            <div className="transfer-header">
              <div className="transfer-id">
                <span className="id-label">Mission ID:</span>
                <span className="id-value">{transfer.session_id.substring(0, 8)}...</span>
              </div>
              <span className={`transfer-status ${getStatusColor(transfer.status)}`}>
                {getStatusText(transfer.status)}
              </span>
            </div>

            {getStatusMessage(transfer) && (
              <div className="status-message">
                <span className="signal-pulse">●</span>
                {getStatusMessage(transfer)}
              </div>
            )}

            <div className="transfer-progress">
              <div className="progress-bar-container">
                <div 
                  className={`progress-bar-fill ${transfer.status === 'Active' ? 'animated' : ''}`}
                  style={{ width: `${transfer.progress_percent || 0}%` }}
                />
              </div>
              <span className="progress-text">
                {(transfer.progress_percent || 0).toFixed(1)}%
              </span>
            </div>

            <div className="transfer-details">
              <div className="detail-item">
                <span className="detail-label">Chunks</span>
                <span className="detail-value">
                  {transfer.completed_chunks} / {transfer.total_chunks}
                </span>
              </div>
              <div className="detail-item">
                <span className="detail-label">Data</span>
                <span className="detail-value">
                  {formatBytes(transfer.bytes_transferred)} / {formatBytes(transfer.total_bytes)}
                </span>
              </div>
              <div className="detail-item">
                <span className="detail-label">Speed</span>
                <span className="detail-value">
                  {transfer.current_speed_bps ? 
                    `${formatBytes(transfer.current_speed_bps)}/s` : 
                    'N/A'}
                </span>
              </div>
            </div>

            <div className="transfer-actions">
              <button 
                className="action-button pause-button"
                onClick={() => onPause(transfer.session_id)}
                disabled={!canPause(transfer.status)}
                title="Pause Transmission"
              >
                <PauseIcon />
                Hold
              </button>
              <button 
                className="action-button resume-button"
                onClick={() => onResume(transfer.session_id)}
                disabled={!canResume(transfer.status)}
                title="Resume Transmission"
              >
                <PlayIcon />
                Resume
              </button>
              <button 
                className="action-button cancel-button"
                onClick={() => {
                  if (window.confirm('Abort this mission? Data will need to be retransmitted.')) {
                    onCancel(transfer.session_id);
                  }
                }}
                title="Abort Mission"
              >
                <CancelIcon />
                Abort
              </button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

export default TransferList;
