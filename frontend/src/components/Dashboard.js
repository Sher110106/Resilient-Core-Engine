import React from 'react';
import './Dashboard.css';

function Dashboard({ stats }) {
  return (
    <div className="dashboard">
      <div className="stat-card active">
        <div className="stat-icon">⚡</div>
        <div className="stat-content">
          <div className="stat-value">{stats.active}</div>
          <div className="stat-label">Active Transfers</div>
        </div>
      </div>

      <div className="stat-card completed">
        <div className="stat-icon">✅</div>
        <div className="stat-content">
          <div className="stat-value">{stats.completed}</div>
          <div className="stat-label">Completed</div>
        </div>
      </div>

      <div className="stat-card failed">
        <div className="stat-icon">❌</div>
        <div className="stat-content">
          <div className="stat-value">{stats.failed}</div>
          <div className="stat-label">Failed</div>
        </div>
      </div>
    </div>
  );
}

export default Dashboard;
