import React from 'react';
import { Pause, Play, X, ArrowRight } from 'lucide-react';
import './TransferList.css';

function TransferList({ transfers, onPause, onResume, onCancel }) {
  const formatBytes = (bytes) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return Math.round(bytes / Math.pow(k, i) * 100) / 100 + ' ' + sizes[i];
  };

  const getStatusClass = (status) => {
    if (status === 'Active') return 'active';
    if (status === 'Completed') return 'completed';
    if (status === 'Paused') return 'paused';
    if (status?.Failed) return 'failed';
    return '';
  };

  const getStatusText = (status) => {
    if (typeof status === 'object' && status.Failed) return 'Failed';
    if (status === 'Active') return 'Transferring';
    if (status === 'Completed') return 'Completed';
    if (status === 'Paused') return 'Paused';
    return status || 'Unknown';
  };

  if (!transfers || transfers.length === 0) {
    return (
      <div className="transfer-list">
        <div className="section-header">
          <h3>Transfers</h3>
        </div>
        <div className="empty-state">
          <ArrowRight size={20} />
          <span>No active transfers</span>
        </div>
      </div>
    );
  }

  return (
    <div className="transfer-list">
      <div className="section-header">
        <h3>Transfers ({transfers.length})</h3>
      </div>

      <div className="transfer-table">
        <div className="table-header">
          <span className="col-id">Session</span>
          <span className="col-status">Status</span>
          <span className="col-progress">Progress</span>
          <span className="col-data">Data</span>
          <span className="col-speed">Speed</span>
          <span className="col-actions">Actions</span>
        </div>

        {transfers.map((transfer) => (
          <div key={transfer.session_id} className={`table-row ${getStatusClass(transfer.status)}`}>
            <span className="col-id mono">{transfer.session_id.substring(0, 8)}</span>

            <span className="col-status">
              <span className={`status-badge ${getStatusClass(transfer.status)}`}>
                {getStatusText(transfer.status)}
              </span>
            </span>

            <span className="col-progress">
              <div className="progress-bar">
                <div
                  className={`progress-fill ${transfer.status === 'Active' ? 'animated' : ''}`}
                  style={{ width: `${transfer.progress_percent || 0}%` }}
                />
              </div>
              <span className="progress-text mono">
                {(transfer.progress_percent || 0).toFixed(0)}%
              </span>
            </span>

            <span className="col-data mono">
              {formatBytes(transfer.bytes_transferred)}
            </span>

            <span className="col-speed mono">
              {transfer.current_speed_bps ? `${formatBytes(transfer.current_speed_bps)}/s` : '--'}
            </span>

            <span className="col-actions">
              {transfer.status === 'Active' && (
                <button className="icon-btn" onClick={() => onPause(transfer.session_id)} title="Pause">
                  <Pause size={14} />
                </button>
              )}
              {transfer.status === 'Paused' && (
                <button className="icon-btn" onClick={() => onResume(transfer.session_id)} title="Resume">
                  <Play size={14} />
                </button>
              )}
              {transfer.status !== 'Completed' && !(typeof transfer.status === 'object' && transfer.status.Failed) && (
                <button
                  className="icon-btn danger"
                  onClick={() => {
                    if (window.confirm('Cancel this transfer?')) {
                      onCancel(transfer.session_id);
                    }
                  }}
                  title="Cancel"
                >
                  <X size={14} />
                </button>
              )}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

export default TransferList;
