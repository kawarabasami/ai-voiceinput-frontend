import React from "react";

interface StatusBarProps {
  text: string;
  color: string;
}

const StatusBar: React.FC<StatusBarProps> = ({ text, color }) => {
  return (
    <div className="status-bar" style={{ "--status-color": color } as React.CSSProperties}>
      <span className="status-indicator" />
      <span className="status-text">{text}</span>
    </div>
  );
};

export default StatusBar;
