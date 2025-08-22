// API utility functions for Google Docs integration

export interface GoogleDocResult {
  document_id: string;
  url: string;
  title: string;
  test_mode?: boolean;
}

export interface CreateGoogleDocResponse {
  success: boolean;
  message: string;
  google_doc: GoogleDocResult;
}

export interface CreateGoogleDocWithAudioResponse {
  success: boolean;
  message: string;
  google_doc: GoogleDocResult;
  transcript_length: number;
  ai_interactions_included: boolean;
  existing_transcript_included: boolean;
  audio_file_path?: string;
}

export interface UploadAudioResponse {
  success: boolean;
  message: string;
  filename: string;
  size: number;
  content_type: string;
  meeting_id?: string;
  transcript: string;
  ai_interactions_included: boolean;
  google_doc?: GoogleDocResult;
}

const API_BASE_URL = 'http://localhost:5167';

export const createGoogleDoc = async (meetingId: string): Promise<CreateGoogleDocResponse> => {
  const response = await fetch(`${API_BASE_URL}/create-google-doc`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      meeting_id: meetingId
    })
  });

  if (!response.ok) {
    throw new Error(`Failed to create Google Doc: ${response.statusText}`);
  }

  return response.json();
};

export const uploadAudioWithMeeting = async (file: File, meetingId?: string): Promise<UploadAudioResponse> => {
  const formData = new FormData();
  formData.append('file', file);
  
  if (meetingId) {
    formData.append('meeting_id', meetingId);
  }

  const response = await fetch(`${API_BASE_URL}/upload-audio`, {
    method: 'POST',
    body: formData
  });

  if (!response.ok) {
    throw new Error(`Failed to upload audio: ${response.statusText}`);
  }

  return response.json();
};

export const createGoogleDocWithAudio = async (
  file: File, 
  meetingId?: string, 
  meetingTitle?: string
): Promise<CreateGoogleDocWithAudioResponse> => {
  const formData = new FormData();
  formData.append('file', file);
  
  if (meetingId) {
    formData.append('meeting_id', meetingId);
  }
  
  if (meetingTitle) {
    formData.append('meeting_title', meetingTitle);
  }

  const response = await fetch(`${API_BASE_URL}/create-google-doc-with-audio`, {
    method: 'POST',
    body: formData
  });

  if (!response.ok) {
    throw new Error(`Failed to create Google Doc with audio: ${response.statusText}`);
  }

  return response.json();
};

export const createGoogleDocWithAudioPath = async (
  audioFilePath: string,
  meetingId?: string,
  meetingTitle?: string
): Promise<CreateGoogleDocWithAudioResponse> => {
  const response = await fetch(`${API_BASE_URL}/create-google-doc-with-audio-path`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      audio_file_path: audioFilePath,
      meeting_id: meetingId,
      meeting_title: meetingTitle
    })
  });

  if (!response.ok) {
    throw new Error(`Failed to create Google Doc with audio path: ${response.statusText}`);
  }

  return response.json();
};
