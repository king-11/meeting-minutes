'use client';

import React, { useState, useEffect } from 'react';
import { Button } from '@/components/ui/button';
import { MessageToast } from '@/components/MessageToast';
import { createGoogleDocWithAudioPath } from '@/config/api';
import { invoke } from '@tauri-apps/api/core';
import { appDataDir } from '@tauri-apps/api/path';

interface AudioGoogleDocButtonProps {
  meetingId?: string;
  meetingTitle?: string;
  disabled?: boolean;
  className?: string;
  recordingPath?: string; // Optional prop to pass recording path
}

export function AudioGoogleDocButton({ meetingId, meetingTitle, disabled = false, className = '', recordingPath }: AudioGoogleDocButtonProps) {
  const [isCreating, setIsCreating] = useState(false);
  const [latestRecordingPath, setLatestRecordingPath] = useState<string | null>(recordingPath || null);
  const [toast, setToast] = useState<{
    isVisible: boolean;
    message: string;
    type: 'success' | 'error' | 'info';
    duration?: number;
  }>({
    isVisible: false,
    message: '',
    type: 'info'
  });

  // Try to find the latest recording file if none provided
  useEffect(() => {
    const findLatestRecording = async () => {
      if (recordingPath) {
        setLatestRecordingPath(recordingPath);
        return;
      }

      try {
        const dataDir = await appDataDir();
        console.log('App data dir:', dataDir);
        
        // Get list of recording files and find the most recent one
        const files = await invoke<string[]>('get_recording_files', { dataDir });
        console.log('Found recording files:', files);
        
        if (files && files.length > 0) {
          // Sort by timestamp (newest first) and take the first one
          const sortedFiles = files.sort((a, b) => b.localeCompare(a));
          // Ensure proper path separator - dataDir should end with '/'
          const separator = dataDir.endsWith('/') ? '' : '/';
          const fullPath = `${dataDir}${separator}${sortedFiles[0]}`;
          console.log('Constructed recording path:', fullPath);
          setLatestRecordingPath(fullPath);
        }
      } catch (error) {
        console.log('Could not find recent recording files:', error);
      }
    };

    findLatestRecording();
  }, [recordingPath]);

  const showToast = (message: string, type: 'success' | 'error' | 'info' = 'info', duration = 5000) => {
    setToast({
      isVisible: true,
      message,
      type,
      duration
    });
  };

  const handleCreateGoogleDoc = async () => {
    if (disabled || isCreating || !latestRecordingPath) return;

    console.log('Creating Google Doc with audio path:', latestRecordingPath);
    console.log('Meeting ID:', meetingId);
    console.log('Meeting Title:', meetingTitle);

    setIsCreating(true);
    try {
      showToast('Processing recorded audio and creating Google Doc...', 'info', 10000);
      
      const response = await createGoogleDocWithAudioPath(latestRecordingPath, meetingId, meetingTitle);
      
      if (response.success && response.google_doc) {
        const { google_doc } = response;
        
        let successMessage = 'Google Doc created successfully with recorded audio transcription!';
        
        // Add details about what was included
        const details = [];
        if (response.transcript_length) {
          details.push(`${response.transcript_length} chars transcribed`);
        }
        if (response.existing_transcript_included) {
          details.push('existing transcript included');
        }
        if (response.ai_interactions_included) {
          details.push('AI interactions included');
        }
        
        if (details.length > 0) {
          successMessage += ` (${details.join(', ')})`;
        }
        
        if (google_doc.test_mode) {
          showToast(
            `Test mode: ${successMessage}. Check console for details.`,
            'info',
            7000
          );
        } else {
          showToast(successMessage, 'success', 6000);
          
          // Open the Google Doc in the default browser
          try {
            await invoke('plugin:shell|open', { path: google_doc.url });
          } catch (error) {
            console.error('Failed to open URL with Tauri shell plugin:', error);
            // Fallback: try to use window.open (might work in dev mode)
            window.open(google_doc.url, '_blank');
          }
        }
      } else {
        showToast('Failed to create Google Doc with recorded audio. Please try again.', 'error');
      }
    } catch (error) {
      console.error('Error creating Google Doc with recorded audio:', error);
      const errorMessage = error instanceof Error ? error.message : 'Unknown error occurred';
      
      if (errorMessage.includes('Audio file not found')) {
        showToast('No recorded audio file found. Please record some audio first.', 'error');
      } else if (errorMessage.includes('Meeting not found')) {
        showToast('Meeting not found. The audio will be processed without meeting context.', 'error');
      } else if (errorMessage.includes('GenAI processing failed')) {
        showToast('Audio transcription failed. Please check your GenAI API key configuration.', 'error');
      } else {
        showToast(`Error creating Google Doc: ${errorMessage}`, 'error');
      }
    } finally {
      setIsCreating(false);
    }
  };

  return (
    <>
      <Button
        onClick={handleCreateGoogleDoc}
        disabled={disabled || isCreating || !latestRecordingPath}
        className={className}
        variant="outline"
        title={!latestRecordingPath ? "No recorded audio found. Please record audio first." : "Create Google Doc with recorded audio"}
      >
        {isCreating ? (
          <>
            <div className="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin mr-2" />
            Processing Audio...
          </>
        ) : (
          <>
            <svg className="w-4 h-4 mr-2" viewBox="0 0 24 24" fill="currentColor">
              <path d="M14,2H6A2,2 0 0,0 4,4V20A2,2 0 0,0 6,22H18A2,2 0 0,0 20,20V8L14,2M18,20H6V4H13V9H18V20Z" />
              <path d="M12,11A3,3 0 0,1 15,14A3,3 0 0,1 12,17A3,3 0 0,1 9,14A3,3 0 0,1 12,11Z" />
            </svg>
            {latestRecordingPath ? 'Recording to Google Doc' : 'No Audio Available'}
          </>
        )}
      </Button>
      
      <MessageToast
        show={toast.isVisible}
        setShow={(show: boolean) => !show && setToast(prev => ({ ...prev, isVisible: false }))}
        message={toast.message}
        type={toast.type}
        duration={toast.duration}
      />
    </>
  );
}
