import React from 'react';
import './TransferList.css';

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
      return 'Failed';
    }
    return status || 'Unknown';
  };

  const canPause = (status) => status === 'Active';
  const canResume = (status) => status === 'Paused';

  if (!transfers || transfers.length === 0) {
    return (
      <div className="transfer-list-container">
        <h2>Active Transfers</h2>
        <div className="empty-state">
          <span className="empty-icon">📦</span>
          <p>No active transfers</p>
          <p className="empty-hint">Upload a file to get started</p>
        </div>
      </div>
    );
  }

  return (
    <div className="transfer-list-container">
      <h2>Active Transfers ({transfers.length})</h2>
      
      <div className="transfer-list">
        {transfers.map((transfer) => (
          <div key={transfer.session_id} className="transfer-item">
            <div className="transfer-header">
              <div className="transfer-id">
                <span className="id-label">Session:</span>
                <span className="id-value">{transfer.session_id.substring(0, 8)}...</span>
              </div>
              <span className={`transfer-status ${getStatusColor(transfer.status)}`}>
                {getStatusText(transfer.status)}
              </span>
            </div>

            <div className="transfer-progress">
              <div className="progress-bar-container">
                <div 
                  className="progress-bar-fill" 
                  style={{ width: `${transfer.progress_percent || 0}%` }}
                >
                  <span className="progress-text">
                    {(transfer.progress_percent || 0).toFixed(1)}%
                  </span>
                </div>
              </div>
            </div>

            <div className="transfer-details">
              <div className="detail-item">
                <span className="detail-label">Chunks:</span>
                <span className="detail-value">
                  {transfer.completed_chunks} / {transfer.total_chunks}
                </span>
              </div>
              <div className="detail-item">
                <span className="detail-label">Data:</span>
                <span className="detail-value">
                  {formatBytes(transfer.bytes_transferred)} / {formatBytes(transfer.total_bytes)}
                </span>
              </div>
              <div className="detail-item">
                <span className="detail-label">Speed:</span>
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
              >
                ⏸️ Pause
              </button>
              <button 
                className="action-button resume-button"
                onClick={() => onResume(transfer.session_id)}
                disabled={!canResume(transfer.status)}
              >
                ▶️ Resume
              </button>
              <button 
                className="action-button cancel-button"
                onClick={() => {
                  if (window.confirm('Are you sure you want to cancel this transfer?')) {
                    onCancel(transfer.session_id);
                  }
                }}
              >
                ❌ Cancel
              </button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

export default TransferList;
