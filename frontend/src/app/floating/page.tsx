'use client';

import { useEffect, useState, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow, LogicalPosition } from '@tauri-apps/api/window';

interface AudioLevels {
  rms: number;
  peak: number;
}

export default function FloatingWindow() {
  const [isRecording, setIsRecording] = useState(false);
  const [audioLevels, setAudioLevels] = useState<AudioLevels>({ rms: 0, peak: 0 });
  const [recordingTime, setRecordingTime] = useState(0);
  const [showSaveConfirmation, setShowSaveConfirmation] = useState(false);
  const startTimeRef = useRef<number>(0);
  const timerRef = useRef<NodeJS.Timeout | null>(null);
  const [currentWindow, setCurrentWindow] = useState<any>(null);

  useEffect(() => {
    // Only run in Tauri environment
    if (typeof window !== 'undefined') {
      const currentWin = getCurrentWindow();
      setCurrentWindow(currentWin);

      // Listen for recording events
      const unlistenStart = listen('start-recording-from-tray', () => {
        console.log('Received start-recording-from-tray event');
        handleStartRecording();
      });

      const unlistenStop = listen('stop-recording-from-tray', () => {
        console.log('Received stop-recording-from-tray event');
        handleStopRecording();
      });

      // Also listen for regular recording events from main window
      const unlistenMainStart = listen('recording-started', () => {
        console.log('Received recording-started event');
        handleStartRecording();
      });

      const unlistenMainStop = listen('recording-stopped', () => {
        console.log('Received recording-stopped event');
        handleStopRecording();
      });

      // Listen for audio level updates
      const unlistenAudioLevels = listen<AudioLevels>('audio-levels', (event) => {
        console.log('Received audio levels:', event.payload);
        setAudioLevels(event.payload);
      });

      return () => {
        unlistenStart.then(fn => fn());
        unlistenStop.then(fn => fn());
        unlistenMainStart.then(fn => fn());
        unlistenMainStop.then(fn => fn());
        unlistenAudioLevels.then(fn => fn());
        if (timerRef.current) {
          clearInterval(timerRef.current);
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
      const position = await invoke<{ x: number; y: number }>('get_window_position');
      if (position) {
        await currentWindow.setPosition(new LogicalPosition(position.x, position.y));
      }
    } catch (error) {
      console.error('Failed to load window position:', error);
    }
  };

  const saveWindowPosition = async (x: number, y: number) => {
    try {
      await invoke('save_window_position', { x, y });
    } catch (error) {
      console.error('Failed to save window position:', error);
    }
  };

  const handleStartRecording = () => {
    console.log('Starting recording in floating window');
    setIsRecording(true);
    setRecordingTime(0);
    startTimeRef.current = Date.now();
    
    // Start timer with 1 second interval for visible updates
    timerRef.current = setInterval(() => {
      const elapsed = Math.floor((Date.now() - startTimeRef.current) / 1000);
      console.log('Timer update:', elapsed);
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
    return `${mins}:${secs.toString().padStart(2, '0')}`;
  };

  const getAudioLevelHeight = (level: number) => {
    return `${Math.min(100, level * 100)}%`;
  };

  return (
    <div 
      className="floating-window"
      style={{
        width: '220px',
        height: '90px',
        backgroundColor: 'rgba(20, 20, 20, 0.9)',
        borderRadius: '12px',
        padding: '12px',
        color: 'white',
        fontFamily: 'system-ui, -apple-system, sans-serif',
        display: 'flex',
        flexDirection: 'column',
        gap: '8px',
        backdropFilter: 'blur(10px)',
        WebkitBackdropFilter: 'blur(10px)',
        boxShadow: '0 8px 32px rgba(0, 0, 0, 0.4), 0 2px 8px rgba(0, 0, 0, 0.2)',
        cursor: 'move',
        userSelect: 'none',
        ...{ WebkitAppRegion: 'drag' } as any,
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
        <div style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          height: '100%',
          fontSize: '14px',
          fontWeight: '500',
        }}>
          ✓ Saved locally
        </div>
      ) : (
        <>
          {/* Header */}
          <div style={{
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'center',
            fontSize: '12px',
            opacity: 0.8,
          }}>
            <span>{isRecording ? 'Recording' : 'Ready'}</span>
            <span>{formatTime(recordingTime)}</span>
          </div>

          {/* Audio Level Meters */}
          <div 
            data-testid="audio-level-meter"
            style={{
              flex: 1,
              display: 'flex',
              gap: '2px',
              alignItems: 'flex-end',
              ...{ WebkitAppRegion: 'no-drag' } as any,
            }}
          >
            {Array.from({ length: 20 }).map((_, i) => (
              <div
                key={i}
                style={{
                  flex: 1,
                  backgroundColor: isRecording 
                    ? (i < audioLevels.rms * 20 
                      ? '#00ff88' 
                      : i < audioLevels.peak * 20 
                        ? '#ffaa00' 
                        : 'rgba(255, 255, 255, 0.15)')
                    : 'rgba(255, 255, 255, 0.1)',
                  borderRadius: '2px',
                  transition: 'height 0.05s ease-out',
                  height: isRecording 
                    ? (i < audioLevels.peak * 20 ? '100%' : '20%')
                    : '20%',
                }}
              />
            ))}
          </div>

          {/* Status */}
          <div style={{
            fontSize: '10px',
            opacity: 0.6,
            textAlign: 'center',
          }}>
            {isRecording ? 'Press Option+Space to stop' : 'Press Option+Space to start'}
          </div>
        </>
      )}
    </div>
  );
}