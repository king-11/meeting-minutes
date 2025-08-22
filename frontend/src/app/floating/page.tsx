"use client";

import { useEffect, useState, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalPosition } from "@tauri-apps/api/window";

interface AudioLevels {
  rms: number;
  peak: number;
}

export default function FloatingWindow() {
  const [isRecording, setIsRecording] = useState(false);
  const [audioLevels, setAudioLevels] = useState<AudioLevels>({
    rms: 0,
    peak: 0,
  });
  const [recordingTime, setRecordingTime] = useState(0);
  const [showSaveConfirmation, setShowSaveConfirmation] = useState(false);
  const startTimeRef = useRef<number>(0);
  const timerRef = useRef<NodeJS.Timeout | null>(null);
  const [currentWindow, setCurrentWindow] = useState<any>(null);
  
  // Refs for high-frequency audio data to avoid excessive re-renders
  const audioLevelsRef = useRef<AudioLevels>({ rms: 0, peak: 0 });
  const animationFrameRef = useRef<number | undefined>(undefined);
  const lastLogTimeRef = useRef<number>(0);

  useEffect(() => {
    // Only run in Tauri environment
    if (typeof window !== "undefined") {
      console.log("[Floating] Initializing floating window event listeners");
      const currentWin = getCurrentWindow();
      setCurrentWindow(currentWin);

      // Also listen for regular recording events from main window
      const unlistenMainStart = listen("recording-started", () => {
        console.log("[Floating] Received recording-started event");
        handleStartRecording();
      });

      const unlistenMainStop = listen("recording-stopped", () => {
        console.log("[Floating] Received recording-stopped event");
        handleStopRecording();
      });

      // Listen for audio level updates with optimized handling
      const unlistenAudioLevels = listen<AudioLevels>(
        "audio-levels",
        (event) => {
          // Debounced logging - only log every 500ms to reduce console spam
          const now = Date.now();
          if (now - lastLogTimeRef.current > 500) {
            console.log("[Floating] Audio levels - RMS:", event.payload.rms.toFixed(3), "Peak:", event.payload.peak.toFixed(3));
            lastLogTimeRef.current = now;
          }
          
          // Store in ref immediately (no re-render)
          audioLevelsRef.current = event.payload;
          
          // Schedule UI update via requestAnimationFrame for smooth 60fps updates
          if (!animationFrameRef.current) {
            animationFrameRef.current = requestAnimationFrame(() => {
              setAudioLevels(audioLevelsRef.current);
              animationFrameRef.current = undefined;
            });
          }
        },
      );

      return () => {
        unlistenMainStart.then((fn) => fn());
        unlistenMainStop.then((fn) => fn());
        unlistenAudioLevels.then((fn) => fn());
        if (timerRef.current) {
          clearInterval(timerRef.current);
        }
        // Clean up any pending animation frame
        if (animationFrameRef.current) {
          cancelAnimationFrame(animationFrameRef.current);
          animationFrameRef.current = undefined;
        }
      };
    }
  }, []);

  // Load window position after currentWindow is set
  useEffect(() => {
    if (currentWindow) {
      // Add a small delay to ensure window is fully initialized
      setTimeout(() => {
        loadWindowPosition();
      }, 100);
    }
  }, [currentWindow]);

  const loadWindowPosition = async () => {
    if (!currentWindow) return;
    try {
      const position = await invoke<{ x: number; y: number }>(
        "get_window_position",
      );
      if (position) {
        await currentWindow.setPosition(
          new LogicalPosition(position.x, position.y),
        );
      }
    } catch (error) {
      console.error("Failed to load window position:", error);
    }
  };

  const saveWindowPosition = async (x: number, y: number) => {
    try {
      await invoke("save_window_position", { x, y });
    } catch (error) {
      console.error("Failed to save window position:", error);
    }
  };

  const handleStartRecording = () => {
    console.log("Starting recording in floating window");
    setIsRecording(true);
    setRecordingTime(0);
    startTimeRef.current = Date.now();

    // Start timer with 1 second interval for visible updates
    timerRef.current = setInterval(() => {
      const elapsed = Math.floor((Date.now() - startTimeRef.current) / 1000);
      console.log("Timer update:", elapsed);
      setRecordingTime(elapsed);
    }, 1000);

    // Show window
    if (currentWindow) {
      currentWindow.show();
    }
  };

  const handleStopRecording = () => {
    setIsRecording(false);

    // Stop timer
    if (timerRef.current) {
      clearInterval(timerRef.current);
      timerRef.current = null;
    }

    // Show save confirmation
    setShowSaveConfirmation(true);

    // Hide window after 2 seconds
    setTimeout(() => {
      setShowSaveConfirmation(false);
      if (currentWindow) {
        currentWindow.hide();
      }
    }, 2000);
  };

  const formatTime = (seconds: number) => {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins}:${secs.toString().padStart(2, "0")}`;
  };

  const getAudioLevelHeight = (level: number) => {
    return `${Math.min(100, level * 100)}%`;
  };

  return (
    <>
      <style jsx>{`
        @keyframes gradientShift {
          0% { background-position: 0% 50%; }
          50% { background-position: 100% 50%; }
          100% { background-position: 0% 50%; }
        }
        
        @keyframes pulse {
          0%, 100% { opacity: 0.8; }
          50% { opacity: 1; }
        }
        
        .floating-window::before {
          content: '';
          position: absolute;
          inset: -2px;
          background: linear-gradient(
            90deg,
            #D8FD49,
            #343CED,
            #E16BFF,
            #DCBB9B,
            #D8FD49
          );
          background-size: 300% 300%;
          animation: gradientShift 8s ease infinite;
          border-radius: 18px;
          opacity: ${isRecording ? 0.8 : 0.4};
          z-index: -1;
          transition: opacity 0.3s ease;
        }
        
        .floating-window {
          position: relative;
        }
      `}</style>
      <div
        className="floating-window"
        style={{
          width: "280px",
          height: "120px",
          background: isRecording 
            ? "linear-gradient(135deg, rgba(52, 60, 237, 0.25), rgba(216, 253, 73, 0.15), rgba(225, 107, 255, 0.1))"
            : "linear-gradient(135deg, rgba(52, 60, 237, 0.15), rgba(225, 223, 215, 0.08))",
          backgroundColor: "rgba(20, 20, 30, 0.6)",
          borderRadius: "16px",
          padding: "16px",
          color: "white",
          fontFamily: "system-ui, -apple-system, sans-serif",
          display: "flex",
          flexDirection: "column",
          gap: "10px",
          backdropFilter: "blur(24px) saturate(200%)",
          WebkitBackdropFilter: "blur(24px) saturate(200%)",
          boxShadow: isRecording
            ? "0 12px 48px rgba(52, 60, 237, 0.4), 0 4px 16px rgba(216, 253, 73, 0.3), 0 0 60px rgba(216, 253, 73, 0.15)"
            : "0 12px 48px rgba(0, 0, 0, 0.5), 0 4px 16px rgba(0, 0, 0, 0.3)",
          border: "1px solid rgba(255, 255, 255, 0.15)",
          cursor: "move",
          userSelect: "none",
          transition: "all 0.3s ease",
          ...({ WebkitAppRegion: "drag" } as any),
        }}
        onMouseUp={async () => {
          // Save position when drag ends
          if (currentWindow) {
            const position = await currentWindow.outerPosition();
            saveWindowPosition(position.x, position.y);
          }
        }}
      >
      {showSaveConfirmation ? (
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            height: "100%",
            fontSize: "14px",
            fontWeight: "500",
            color: "#D8FD49",
            textShadow: "0 0 20px rgba(216, 253, 73, 0.5)",
          }}
        >
          ✓ Saved locally
        </div>
      ) : (
        <>
          {/* Header */}
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              fontSize: "14px",
              opacity: 0.95,
              fontWeight: "600",
              letterSpacing: "0.5px",
            }}
          >
            <span style={{
              color: isRecording ? "#D8FD49" : "#E1DFD7",
              animation: isRecording ? "pulse 2s ease infinite" : "none",
              display: "flex",
              alignItems: "center",
              gap: "6px",
            }}>
              {isRecording && <span style={{ fontSize: "10px" }}>●</span>}
              {isRecording ? "Recording" : "Ready"}
            </span>
            <span style={{ 
              color: "#ffffff",
              fontSize: "16px",
              fontWeight: "500",
            }}>{formatTime(recordingTime)}</span>
          </div>

          {/* Audio Level Meters */}
          <div
            data-testid="audio-level-meter"
            style={{
              display: "flex",
              gap: "3px",
              alignItems: "flex-end",
              padding: "4px",
              background: "rgba(0, 0, 0, 0.25)",
              borderRadius: "8px",
              height: "36px",
              minHeight: "36px",
              maxHeight: "36px",
              ...({ WebkitAppRegion: "no-drag" } as any),
            }}
          >
            {Array.from({ length: 25 }).map((_, i) => {
              const normalizedIndex = i / 25;
              const isActive = isRecording && i < audioLevels.peak * 25;
              const isRMS = isRecording && i < audioLevels.rms * 25;
              
              return (
                <div
                  key={i}
                  style={{
                    flex: 1,
                    background: isRecording
                      ? isRMS
                        ? `linear-gradient(to top, #343CED, #D8FD49)`
                        : isActive
                          ? `linear-gradient(to top, #343CED, #E16BFF)`
                          : "rgba(225, 223, 215, 0.12)"
                      : "rgba(225, 223, 215, 0.1)",
                    borderRadius: "3px",
                    transition: "all 0.1s ease-out",
                    height: isRecording
                      ? isActive
                        ? "100%"
                        : "25%"
                      : "25%",
                    boxShadow: isActive
                      ? normalizedIndex > 0.8
                        ? "0 0 10px rgba(225, 107, 255, 0.7)"
                        : normalizedIndex > 0.5
                          ? "0 0 8px rgba(216, 253, 73, 0.5)"
                          : "0 0 6px rgba(52, 60, 237, 0.4)"
                      : "none",
                  }}
                />
              );
            })}
          </div>

          {/* Status */}
          <div
            style={{
              fontSize: "11px",
              opacity: 0.75,
              textAlign: "center",
              color: "#E1DFD7",
              letterSpacing: "0.4px",
              fontWeight: "400",
            }}
          >
            {isRecording
              ? "Press Option+Space to stop"
              : "Press Option+Space to start"}
          </div>
        </>
      )}
    </div>
    </>
  );
}
