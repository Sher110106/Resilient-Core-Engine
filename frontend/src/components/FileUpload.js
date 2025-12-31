import React, { useState } from 'react';
import './FileUpload.css';

function FileUpload({ onUpload }) {
  const [selectedFile, setSelectedFile] = useState(null);
  const [priority, setPriority] = useState('Normal');
  const [dragActive, setDragActive] = useState(false);

  const handleDrag = (e) => {
    e.preventDefault();
    e.stopPropagation();
    if (e.type === "dragenter" || e.type === "dragover") {
      setDragActive(true);
    } else if (e.type === "dragleave") {
      setDragActive(false);
    }
  };

  const handleDrop = (e) => {
    e.preventDefault();
    e.stopPropagation();
    setDragActive(false);
    
    if (e.dataTransfer.files && e.dataTransfer.files[0]) {
      setSelectedFile(e.dataTransfer.files[0]);
    }
  };

  const handleChange = (e) => {
    e.preventDefault();
    if (e.target.files && e.target.files[0]) {
      setSelectedFile(e.target.files[0]);
    }
  };

  const handleSubmit = async (e) => {
    e.preventDefault();
    if (selectedFile) {
      // Note: In a real implementation, you'd need to upload the file to the server first
      // For now, we'll pass the file object and the backend will need to handle it
      await onUpload(selectedFile, priority);
      setSelectedFile(null);
    }
  };

  return (
    <div className="file-upload-container">
      <h2>Upload File</h2>
      
      <form onSubmit={handleSubmit} className="file-upload-form">
        <div 
          className={`drop-zone ${dragActive ? 'drag-active' : ''}`}
          onDragEnter={handleDrag}
          onDragLeave={handleDrag}
          onDragOver={handleDrag}
          onDrop={handleDrop}
        >
          <input
            type="file"
            id="file-input"
            onChange={handleChange}
            className="file-input"
          />
          <label htmlFor="file-input" className="file-label">
            {selectedFile ? (
              <div className="file-info">
                <span className="file-icon">📄</span>
                <span className="file-name">{selectedFile.name}</span>
                <span className="file-size">
                  {(selectedFile.size / 1024).toFixed(2)} KB
                </span>
              </div>
            ) : (
              <div className="upload-prompt">
                <span className="upload-icon">📁</span>
                <p>Drag & drop a file here or click to browse</p>
              </div>
            )}
          </label>
        </div>

        <div className="priority-selector">
          <label htmlFor="priority">Priority:</label>
          <select 
            id="priority"
            value={priority}
            onChange={(e) => setPriority(e.target.value)}
          >
            <option value="Critical">Critical</option>
            <option value="High">High</option>
            <option value="Normal">Normal</option>
          </select>
        </div>

        <button 
          type="submit" 
          className="upload-button"
          disabled={!selectedFile}
        >
          Start Transfer
        </button>
      </form>
    </div>
  );
}

export default FileUpload;
