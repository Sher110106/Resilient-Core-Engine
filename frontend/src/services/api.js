import axios from 'axios';

// In production (behind nginx), REACT_APP_API_URL is set to "" so we use
// relative URLs. In dev, it's unset so we fall back to localhost:3000.
const envUrl = process.env.REACT_APP_API_URL;
const API_BASE_URL = envUrl != null ? envUrl : 'http://localhost:3000';

const api = {
  async healthCheck() {
    const response = await axios.get(`${API_BASE_URL}/health`);
    return response.data;
  },

  async uploadAndTransfer(file, priority = 'Normal', receiverAddr = null) {
    const formData = new FormData();
    formData.append('file', file);
    formData.append('priority', priority);
    if (receiverAddr) {
      formData.append('receiver_addr', receiverAddr);
    }
    const response = await axios.post(`${API_BASE_URL}/api/v1/upload`, formData);
    return response.data;
  },

  async startTransfer(filePath, priority = 'Normal') {
    const response = await axios.post(`${API_BASE_URL}/api/v1/transfers`, {
      file_path: filePath,
      priority: priority
    });
    return response.data;
  },

  async listTransfers() {
    const response = await axios.get(`${API_BASE_URL}/api/v1/transfers`);
    return response.data;
  },

  async getTransferState(sessionId) {
    const response = await axios.get(`${API_BASE_URL}/api/v1/transfers/${sessionId}`);
    return response.data;
  },

  async getProgress(sessionId) {
    const response = await axios.get(`${API_BASE_URL}/api/v1/transfers/${sessionId}/progress`);
    return response.data;
  },

  async pauseTransfer(sessionId) {
    const response = await axios.post(`${API_BASE_URL}/api/v1/transfers/${sessionId}/pause`);
    return response.data;
  },

  async resumeTransfer(sessionId) {
    const response = await axios.post(`${API_BASE_URL}/api/v1/transfers/${sessionId}/resume`);
    return response.data;
  },

  async cancelTransfer(sessionId) {
    const response = await axios.post(`${API_BASE_URL}/api/v1/transfers/${sessionId}/cancel`);
    return response.data;
  },

  // Metric endpoints
  async getErasureMetrics() {
    const response = await axios.get(`${API_BASE_URL}/api/v1/metrics/erasure`);
    return response.data;
  },

  async getNetworkMetrics() {
    const response = await axios.get(`${API_BASE_URL}/api/v1/metrics/network`);
    return response.data;
  },

  async getQueueMetrics() {
    const response = await axios.get(`${API_BASE_URL}/api/v1/metrics/queue`);
    return response.data;
  },

  async getMetricsSummary() {
    const response = await axios.get(`${API_BASE_URL}/api/v1/metrics/summary`);
    return response.data;
  },

  // Simulation
  async simulatePacketLoss(lossRate, filePath = null, durationSeconds = null) {
    const payload = {
      loss_rate: lossRate,
      duration_seconds: durationSeconds
    };
    if (filePath) {
      payload.file_path = filePath;
    }
    const response = await axios.post(`${API_BASE_URL}/api/v1/simulate/packet-loss`, payload);
    return response.data;
  },

  // Uploads
  async listUploads() {
    const response = await axios.get(`${API_BASE_URL}/api/v1/uploads`);
    return response.data;
  },

  // Comparison simulation
  async simulateComparison(filePath, trialsPerPoint = 20) {
    const response = await axios.post(`${API_BASE_URL}/api/v1/simulate/comparison`, {
      file_path: filePath,
      trials_per_point: trialsPerPoint
    });
    return response.data;
  }
};

export default api;
