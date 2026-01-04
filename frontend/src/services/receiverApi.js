import axios from 'axios';

class ReceiverApi {
  constructor(baseUrl = 'http://localhost:8080') {
    this.baseUrl = baseUrl;
  }

  setBaseUrl(url) {
    this.baseUrl = url;
  }

  async getStatus() {
    const response = await axios.get(`${this.baseUrl}/api/v1/receiver/status`);
    return response.data;
  }

  async listFiles() {
    const response = await axios.get(`${this.baseUrl}/api/v1/receiver/files`);
    return response.data;
  }

  getDownloadUrl(filename) {
    return `${this.baseUrl}/api/v1/receiver/files/${encodeURIComponent(filename)}`;
  }
}

export default new ReceiverApi();
