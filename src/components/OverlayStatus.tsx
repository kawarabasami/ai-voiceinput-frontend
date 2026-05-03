import React, { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";

interface OverlayPayload {
  text: string;
  color: string;
}

const OverlayStatus: React.FC = () => {
  const [visible, setVisible] = useState(false);
  const [text, setText] = useState("");
  const [color, setColor] = useState("#fff");

  useEffect(() => {
    const unlistenShow = listen<OverlayPayload>("overlay-show", (event) => {
      console.log("[Overlay] overlay-show received", event.payload);
      setText(event.payload.text);
      setColor(event.payload.color);
      setVisible(true);
    });

    const unlistenHide = listen("overlay-hide", () => {
      console.log("[Overlay] overlay-hide received");
      setVisible(false);
    });

    const unlistenTheme = listen<string>("theme-changed", (event) => {
      console.log("[Overlay] theme-changed received:", event.payload);
      document.documentElement.setAttribute("data-theme", event.payload);
    });

    return () => {
      unlistenShow.then((fn) => fn());
      unlistenHide.then((fn) => fn());
      unlistenTheme.then((fn) => fn());
    };
  }, []);

  if (!visible) return null;

  return (
    <div
      className="overlay-status"
      style={{ "--overlay-color": color } as React.CSSProperties}
    >
      <span className="overlay-dot" />
      <span className="overlay-text">{text}</span>
    </div>
  );
};

export default OverlayStatus;
