'use client';

import React, { useState } from 'react';
import { Button } from '@/components/ui/button';
import { MessageToast } from '@/components/MessageToast';
import { createGoogleDoc } from '@/config/api';
import { invoke } from '@tauri-apps/api/core';
import type { GoogleDocResult } from '@/types';

interface GoogleDocButtonProps {
  meetingId: string;
  meetingTitle: string;
  disabled?: boolean;
  className?: string;
}

export function GoogleDocButton({ meetingId, meetingTitle, disabled = false, className = '' }: GoogleDocButtonProps) {
  const [isCreating, setIsCreating] = useState(false);
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

  const showToast = (message: string, type: 'success' | 'error' | 'info' = 'info', duration = 5000) => {
    setToast({
      isVisible: true,
      message,
      type,
      duration
    });
  };

  const handleCreateGoogleDoc = async () => {
    if (disabled || isCreating) return;

    setIsCreating(true);
    try {
      const response = await createGoogleDoc(meetingId);

      console.log('tichnas response', response);
      
      if (response.success && response.google_doc) {
        console.log('tichnas 1')
        const { google_doc } = response;
        
        if (google_doc.test_mode) {
        console.log('tichnas 2')
          showToast(
            `Test mode: Google Doc would be created for "${meetingTitle}". Check console for details.`,
            'info',
            7000
          );
        } else {
          console.log('tichnas 3')
          showToast(
            `Google Doc created successfully! Opening in new tab...`,
            'success',
            5000
          );

          console.log('tichnas 4')
          
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
        showToast('Failed to create Google Doc. Please try again.', 'error');
      }
    } catch (error) {
      console.error('Error creating Google Doc:', error);
      const errorMessage = error instanceof Error ? error.message : 'Unknown error occurred';
      
      if (errorMessage.includes('Meeting not found')) {
        showToast('Meeting not found. Please ensure the meeting exists and try again.', 'error');
      } else if (errorMessage.includes('No transcript found')) {
        showToast('No transcript available for this meeting. Record some content first.', 'error');
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
        disabled={disabled || isCreating}
        className={className}
        variant="outline"
      >
        {isCreating ? (
          <>
            <div className="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin mr-2" />
            Creating Doc...
          </>
        ) : (
          <>
            <svg className="w-4 h-4 mr-2" viewBox="0 0 24 24" fill="currentColor">
              <path d="M14,2H6A2,2 0 0,0 4,4V20A2,2 0 0,0 6,22H18A2,2 0 0,0 20,20V8L14,2M18,20H6V4H13V9H18V20Z" />
            </svg>
            Create Google Doc
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
