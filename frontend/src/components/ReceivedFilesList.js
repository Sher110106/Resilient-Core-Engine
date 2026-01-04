import React from 'react';
import './ReceivedFilesList.css';

function ReceivedFilesList({ files, receiverUrl }) {
  const formatBytes = (bytes) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return Math.round(bytes / Math.pow(k, i) * 100) / 100 + ' ' + sizes[i];
  };

  const formatDate = (dateString) => {
    try {
      const date = new Date(dateString);
      return date.toLocaleString();
    } catch {
      return dateString;
    }
  };

  const handleDownload = (filename) => {
    const downloadUrl = `${receiverUrl}/api/v1/receiver/files/${encodeURIComponent(filename)}`;
    window.open(downloadUrl, '_blank');
  };

  if (!files || files.length === 0) {
    return (
      <div className="received-files-container">
        <h2>Received Files</h2>
        <div className="empty-state">
          <span className="empty-icon">📭</span>
          <p>No files received yet</p>
          <p className="empty-hint">Files will appear here after transfer completes</p>
        </div>
      </div>
    );
  }

  return (
    <div className="received-files-container">
      <h2>Received Files ({files.length})</h2>
      
      <div className="received-files-list">
        {files.map((file, index) => (
          <div key={index} className="received-file-item">
            <div className="file-icon">
              {file.verified ? '✅' : '⚠️'}
            </div>
            
            <div className="file-info">
              <div className="file-name">{file.filename}</div>
              <div className="file-meta">
                <span className="file-size">{formatBytes(file.size)}</span>
                <span className="file-separator">•</span>
                <span className="file-date">{formatDate(file.received_at)}</span>
                <span className="file-separator">•</span>
                <span className={`file-status ${file.verified ? 'verified' : 'unverified'}`}>
                  {file.verified ? 'Verified' : 'Unverified'}
                </span>
              </div>
            </div>

            <button
              className="download-button"
              onClick={() => handleDownload(file.filename)}
              title="Download file"
            >
              ⬇️ Download
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}

export default ReceivedFilesList;
