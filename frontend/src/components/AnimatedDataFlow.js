import React, { useState, useEffect, useRef } from 'react';
import './AnimatedDataFlow.css';

function AnimatedDataFlow({ currentMetrics }) {
  const [packets, setPackets] = useState([]);
  const counters = useRef({ sent: 0, lost: 0, recovered: 0 });
  const nextId = useRef(0);
  const lossRate = currentMetrics ? currentMetrics.loss_rate : 0;
  const parity = currentMetrics ? currentMetrics.parity_shards : 5;
  const dataShards = currentMetrics ? currentMetrics.data_shards : 50;

  // Recovery capability: can we recover at the current loss rate?
  // Reed-Solomon can tolerate up to parity/(data+parity) loss.
  const maxTolerance = parity / (dataShards + parity);
  const canRecover = lossRate <= maxTolerance;

  useEffect(() => {
    const interval = setInterval(() => {
      const id = nextId.current++;
      const isLost = Math.random() < lossRate;

      let status;
      if (!isLost) {
        status = 'sent';
        counters.current.sent++;
      } else if (canRecover) {
        // Within recovery range: all lost packets are recoverable
        status = 'recovered';
        counters.current.recovered++;
      } else {
        // Beyond recovery range: some lost packets can't be recovered
        // Probabilistically: the fraction within tolerance is recovered,
        // the excess is permanently lost
        const recoveryChance = maxTolerance / lossRate;
        if (Math.random() < recoveryChance) {
          status = 'recovered';
          counters.current.recovered++;
        } else {
          status = 'lost';
          counters.current.lost++;
        }
      }

      setPackets(prev => {
        const next = [...prev, { id, status }];
        if (next.length > 40) next.shift();
        return next;
      });
    }, 150);

    return () => clearInterval(interval);
  }, [lossRate, parity, canRecover, maxTolerance]);

  // Reset counters when loss rate changes significantly
  const prevLoss = useRef(lossRate);
  useEffect(() => {
    if (Math.abs(prevLoss.current - lossRate) > 0.05) {
      counters.current = { sent: 0, lost: 0, recovered: 0 };
      prevLoss.current = lossRate;
    }
  }, [lossRate]);

  const { sent, lost, recovered } = counters.current;

  return (
    <div className="data-flow">
      <div className="flow-labels">
        <div className="flow-label-group">
          <span className="flow-label">SENDER</span>
        </div>
        <div className="flow-stats">
          <span className="flow-stat">
            <span className="flow-dot sent" />
            Delivered: {sent + recovered}
          </span>
          <span className="flow-stat">
            <span className="flow-dot lost" />
            Lost: {lost}
          </span>
          <span className="flow-stat">
            <span className="flow-dot recovered" />
            Recovered: {recovered}
          </span>
        </div>
        <div className="flow-label-group">
          <span className="flow-label">RECEIVER</span>
        </div>
      </div>

      <div className="flow-track">
        <div className="flow-line" />
        {packets.map(packet => {
          const age = nextId.current - packet.id;
          const progress = Math.min(age / 25, 1);

          return (
            <div
              key={packet.id}
              className={`flow-packet ${packet.status}`}
              style={{
                left: `${progress * 100}%`,
                opacity: progress > 0.9 ? (1 - (progress - 0.9) * 10) : (progress < 0.1 ? progress * 10 : 1),
              }}
            />
          );
        })}
      </div>

      {!canRecover && lossRate > 0 && (
        <div className="flow-warning">
          Loss rate ({(lossRate * 100).toFixed(0)}%) exceeds recovery threshold ({(maxTolerance * 100).toFixed(0)}%) -- permanent data loss occurring
        </div>
      )}
    </div>
  );
}

export default AnimatedDataFlow;
