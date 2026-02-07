import React from 'react';
import { FileText, Download, ShieldCheck, Inbox } from 'lucide-react';
import './ReceivedFilesList.css';

function ReceivedFilesList({ files, receiverUrl }) {
  const formatBytes = (bytes) => {
    if (!bytes) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return Math.round(bytes / Math.pow(k, i) * 100) / 100 + ' ' + sizes[i];
  };

  const formatDate = (timestamp) => {
    if (!timestamp) return '--';
    const date = new Date(timestamp * 1000);
    return date.toLocaleString('en-US', {
      month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit'
    });
  };

  if (!files || files.length === 0) {
    return (
      <div className="received-files">
        <div className="section-header">
          <h3>Received Files</h3>
        </div>
        <div className="empty-state">
          <Inbox size={20} />
          <span>No files received yet</span>
        </div>
      </div>
    );
  }

  return (
    <div className="received-files">
      <div className="section-header">
        <h3>Received Files ({files.length})</h3>
      </div>

      <div className="files-list">
        {files.map((file, index) => (
          <div key={index} className="file-row">
            <div className="file-row-icon">
              <FileText size={16} />
            </div>
            <div className="file-row-info">
              <span className="file-row-name">{file.name || file.filename || 'Unknown'}</span>
              <span className="file-row-meta mono">
                {formatBytes(file.size)} -- {formatDate(file.received_at || file.timestamp)}
              </span>
            </div>
            <div className="file-row-badge">
              <ShieldCheck size={12} />
              <span>BLAKE3</span>
            </div>
            <a
              href={`${receiverUrl}/api/v1/receiver/files/${encodeURIComponent(file.name || file.filename)}`}
              className="file-row-download"
              download
            >
              <Download size={14} />
            </a>
          </div>
        ))}
      </div>
    </div>
  );
}

export default ReceivedFilesList;
