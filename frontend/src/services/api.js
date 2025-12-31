import axios from 'axios';

const API_BASE_URL = process.env.REACT_APP_API_URL || 'http://localhost:3000';

const api = {
  // Health check
  async healthCheck() {
    const response = await axios.get(`${API_BASE_URL}/health`);
    return response.data;
  },

  // Upload file and start transfer
  async uploadAndTransfer(file, priority = 'Normal') {
    const formData = new FormData();
    formData.append('file', file);
    formData.append('priority', priority);

    const response = await axios.post(`${API_BASE_URL}/api/v1/upload`, formData, {
      headers: {
        'Content-Type': 'multipart/form-data',
      },
    });
    return response.data;
  },

  // Start a new transfer (for existing files on server)
  async startTransfer(filePath, priority = 'Normal') {
    const response = await axios.post(`${API_BASE_URL}/api/v1/transfers`, {
      file_path: filePath,
      priority: priority
    });
    return response.data;
  },

  // List all active transfers
  async listTransfers() {
    const response = await axios.get(`${API_BASE_URL}/api/v1/transfers`);
    return response.data;
  },

  // Get transfer state
  async getTransferState(sessionId) {
    const response = await axios.get(`${API_BASE_URL}/api/v1/transfers/${sessionId}`);
    return response.data;
  },

  // Get transfer progress
  async getProgress(sessionId) {
    const response = await axios.get(`${API_BASE_URL}/api/v1/transfers/${sessionId}/progress`);
    return response.data;
  },

  // Pause a transfer
  async pauseTransfer(sessionId) {
    const response = await axios.post(`${API_BASE_URL}/api/v1/transfers/${sessionId}/pause`);
    return response.data;
  },

  // Resume a transfer
  async resumeTransfer(sessionId) {
    const response = await axios.post(`${API_BASE_URL}/api/v1/transfers/${sessionId}/resume`);
    return response.data;
  },

  // Cancel a transfer
  async cancelTransfer(sessionId) {
    const response = await axios.post(`${API_BASE_URL}/api/v1/transfers/${sessionId}/cancel`);
    return response.data;
  }
};

export default api;
