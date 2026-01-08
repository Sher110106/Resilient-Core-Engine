import React from 'react';
import './Dashboard.css';

// Activity Icon
const ActivityIcon = () => (
  <svg className="stat-icon-svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/>
  </svg>
);

// Check Circle Icon
const CheckIcon = () => (
  <svg className="stat-icon-svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/>
    <polyline points="22 4 12 14.01 9 11.01"/>
  </svg>
);

// Alert Icon
const AlertIcon = () => (
  <svg className="stat-icon-svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <circle cx="12" cy="12" r="10"/>
    <line x1="12" y1="8" x2="12" y2="12"/>
    <line x1="12" y1="16" x2="12.01" y2="16"/>
  </svg>
);

function Dashboard({ stats }) {
  return (
    <div className="dashboard">
      <div className="stat-card active">
        <div className="stat-icon-wrapper active">
          <ActivityIcon />
        </div>
        <div className="stat-content">
          <div className="stat-value">{stats.active}</div>
          <div className="stat-label">Active Missions</div>
        </div>
      </div>

      <div className="stat-card completed">
        <div className="stat-icon-wrapper completed">
          <CheckIcon />
        </div>
        <div className="stat-content">
          <div className="stat-value">{stats.completed}</div>
          <div className="stat-label">Intel Delivered</div>
        </div>
      </div>

      <div className="stat-card failed">
        <div className="stat-icon-wrapper failed">
          <AlertIcon />
        </div>
        <div className="stat-content">
          <div className="stat-value">{stats.failed}</div>
          <div className="stat-label">Failed Transmissions</div>
        </div>
      </div>
    </div>
  );
}

export default Dashboard;
