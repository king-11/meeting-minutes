from fastapi import FastAPI, HTTPException, BackgroundTasks, WebSocket, WebSocketDisconnect
from fastapi import FastAPI, HTTPException, BackgroundTasks, File, UploadFile
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse
from pydantic import BaseModel
import uvicorn
from typing import Optional, List, Dict, Any
import logging
from dotenv import load_dotenv
from db import DatabaseManager
import json
from threading import Lock
from transcript_processor import TranscriptProcessor
import time
import asyncio
from datetime import datetime
from custom_ai_service import get_ai_service, cleanup_ai_service
import os
from googleapiclient.discovery import build
from google.oauth2 import service_account
from google_auth_oauthlib.flow import InstalledAppFlow
from google.auth.transport.requests import Request
import pickle
import tempfile
from google import genai

# Load environment variables
load_dotenv()

# Configure logger with line numbers and function names
logger = logging.getLogger(__name__)
logger.setLevel(logging.DEBUG)

# Create console handler with formatting
console_handler = logging.StreamHandler()
console_handler.setLevel(logging.DEBUG)

# Create formatter with line numbers and function names
formatter = logging.Formatter(
    '%(asctime)s - %(levelname)s - [%(filename)s:%(lineno)d - %(funcName)s()] - %(message)s',
    datefmt='%Y-%m-%d %H:%M:%S'
)
console_handler.setFormatter(formatter)

# Add handler to logger if not already added
if not logger.handlers:
    logger.addHandler(console_handler)

app = FastAPI(
    title="Meeting Summarizer API",
    description="API for processing and summarizing meeting transcripts",
    version="1.0.0"
)

# Configure CORS
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],     # Allow all origins for testing
    allow_credentials=True,
    allow_methods=["*"],     # Allow all methods
    allow_headers=["*"],     # Allow all headers
    max_age=3600,            # Cache preflight requests for 1 hour
)

# Global database manager instance for meeting management endpoints
db = DatabaseManager()

# class WebSocketConnectionManager:
#     """Manages WebSocket connections for real-time communication"""
#     def __init__(self):
#         self.active_connections: Dict[str, List[WebSocket]] = {}
#         self.lock = Lock()
    
#     async def connect(self, websocket: WebSocket, meeting_id: str):
#         await websocket.accept()
#         with self.lock:
#             if meeting_id not in self.active_connections:
#                 self.active_connections[meeting_id] = []
#             self.active_connections[meeting_id].append(websocket)
#         logger.info(f"WebSocket connected for meeting {meeting_id}")
    
#     def disconnect(self, websocket: WebSocket, meeting_id: str):
#         with self.lock:
#             if meeting_id in self.active_connections:
#                 self.active_connections[meeting_id].remove(websocket)
#                 if not self.active_connections[meeting_id]:
#                     del self.active_connections[meeting_id]
#         logger.info(f"WebSocket disconnected for meeting {meeting_id}")
    
#     async def send_to_meeting(self, meeting_id: str, message: dict):
#         """Send message to all connections for a meeting"""
#         if meeting_id in self.active_connections:
#             disconnected = []
#             for connection in self.active_connections[meeting_id]:
#                 try:
#                     await connection.send_json(message)
#                 except Exception as e:
#                     logger.error(f"Error sending to websocket: {e}")
#                     disconnected.append(connection)
            
#             # Clean up disconnected sockets
#             for conn in disconnected:
#                 self.disconnect(conn, meeting_id)
    
#     async def broadcast(self, message: dict):
#         """Broadcast message to all connected clients"""
#         for meeting_id in list(self.active_connections.keys()):
#             await self.send_to_meeting(meeting_id, message)

# # Initialize WebSocket manager
# ws_manager = WebSocketConnectionManager()

# # Initialize AI service
ai_service = get_ai_service()

# New Pydantic models for meeting management
class Transcript(BaseModel):
    id: str
    text: str
    timestamp: str

class MeetingResponse(BaseModel):
    id: str
    title: str

class MeetingDetailsResponse(BaseModel):
    id: str
    title: str
    created_at: str
    updated_at: str
    transcripts: List[Transcript]

class MeetingTitleUpdate(BaseModel):
    meeting_id: str
    title: str

class DeleteMeetingRequest(BaseModel):
    meeting_id: str

class SaveTranscriptRequest(BaseModel):
    meeting_title: str
    transcripts: List[Transcript]

class SaveModelConfigRequest(BaseModel):
    provider: str
    model: str
    whisperModel: str
    apiKey: Optional[str] = None

class SaveTranscriptConfigRequest(BaseModel):
    provider: str
    model: str
    apiKey: Optional[str] = None

class RealtimeTranscriptRequest(BaseModel):
    """Request model for real-time transcript processing"""
    meeting_id: str
    transcript_chunk: str
    timestamp: Optional[str] = None
    include_context: bool = True

class TranscriptRequest(BaseModel):
    """Request model for transcript text, updated with meeting_id"""
    text: str
    model: str
    model_name: str
    meeting_id: str
    chunk_size: Optional[int] = 5000
    overlap: Optional[int] = 1000
    custom_prompt: Optional[str] = "Generate a summary of the meeting transcript."

class SummaryProcessor:
    """Handles the processing of summaries in a thread-safe way"""
    def __init__(self):
        try:
            self.db = DatabaseManager()

            logger.info("Initializing SummaryProcessor components")
            self.transcript_processor = TranscriptProcessor()
            logger.info("SummaryProcessor initialized successfully (core components)")
        except Exception as e:
            logger.error(f"Failed to initialize SummaryProcessor: {str(e)}", exc_info=True)
            raise

    async def process_transcript(self, text: str, model: str, model_name: str, chunk_size: int = 5000, overlap: int = 1000, custom_prompt: str = "Generate a summary of the meeting transcript.") -> tuple:
        """Process a transcript text"""
        try:
            if not text:
                raise ValueError("Empty transcript text provided")

            # Validate chunk_size and overlap
            if chunk_size <= 0:
                raise ValueError("chunk_size must be positive")
            if overlap < 0:
                raise ValueError("overlap must be non-negative")
            if overlap >= chunk_size:
                overlap = chunk_size - 1  # Ensure overlap is less than chunk_size

            # Ensure step size is positive
            step_size = chunk_size - overlap
            if step_size <= 0:
                chunk_size = overlap + 1  # Adjust chunk_size to ensure positive step

            logger.info(f"Processing transcript of length {len(text)} with chunk_size={chunk_size}, overlap={overlap}")
            num_chunks, all_json_data = await self.transcript_processor.process_transcript(
                text=text,
                model=model,
                model_name=model_name,
                chunk_size=chunk_size,
                overlap=overlap,
                custom_prompt=custom_prompt
            )
            logger.info(f"Successfully processed transcript into {num_chunks} chunks")

            return num_chunks, all_json_data
        except Exception as e:
            logger.error(f"Error processing transcript: {str(e)}", exc_info=True)
            raise

    def cleanup(self):
        """Cleanup resources"""
        try:
            logger.info("Cleaning up resources")
            if hasattr(self, 'transcript_processor'):
                self.transcript_processor.cleanup()
            logger.info("Cleanup completed successfully")
        except Exception as e:
            logger.error(f"Error during cleanup: {str(e)}", exc_info=True)

# Initialize processor
processor = SummaryProcessor()

# New meeting management endpoints
@app.get("/get-meetings", response_model=List[MeetingResponse])
async def get_meetings():
    """Get all meetings with their basic information"""
    try:
        meetings = await db.get_all_meetings()
        return [{"id": meeting["id"], "title": meeting["title"]} for meeting in meetings]
    except Exception as e:
        logger.error(f"Error getting meetings: {str(e)}", exc_info=True)
        raise HTTPException(status_code=500, detail=str(e))

@app.get("/get-meeting/{meeting_id}", response_model=MeetingDetailsResponse)
async def get_meeting(meeting_id: str):
    """Get a specific meeting by ID with all its details"""
    try:
        meeting = await db.get_meeting(meeting_id)
        if not meeting:
            raise HTTPException(status_code=404, detail="Meeting not found")
        return meeting
    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error getting meeting: {str(e)}", exc_info=True)
        raise HTTPException(status_code=500, detail=str(e))

@app.post("/save-meeting-title")
async def save_meeting_title(data: MeetingTitleUpdate):
    """Save a meeting title"""
    try:
        await db.update_meeting_title(data.meeting_id, data.title)
        return {"message": "Meeting title saved successfully"}
    except Exception as e:
        logger.error(f"Error saving meeting title: {str(e)}", exc_info=True)
        raise HTTPException(status_code=500, detail=str(e))

@app.post("/delete-meeting")
async def delete_meeting(data: DeleteMeetingRequest):
    """Delete a meeting and all its associated data"""
    try:
        success = await db.delete_meeting(data.meeting_id)
        if success:
            return {"message": "Meeting deleted successfully"}
        else:
            raise HTTPException(status_code=500, detail="Failed to delete meeting")
    except Exception as e:
        logger.error(f"Error deleting meeting: {str(e)}", exc_info=True)
        raise HTTPException(status_code=500, detail=str(e))

async def process_transcript_background(process_id: str, transcript: TranscriptRequest, custom_prompt: str):
    """Background task to process transcript"""
    try:
        logger.info(f"Starting background processing for process_id: {process_id}")
        
        # Early validation for common issues
        if not transcript.text or not transcript.text.strip():
            raise ValueError("Empty transcript text provided")
        
        if transcript.model in ["claude", "groq", "openai"]:
            # Check if API key is available for cloud providers
            api_key = await processor.db.get_api_key(transcript.model)
            if not api_key:
                provider_names = {"claude": "Anthropic", "groq": "Groq", "openai": "OpenAI"}
                raise ValueError(f"{provider_names.get(transcript.model, transcript.model)} API key not configured. Please set your API key in the model settings.")

        _, all_json_data = await processor.process_transcript(
            text=transcript.text,
            model=transcript.model,
            model_name=transcript.model_name,
            chunk_size=transcript.chunk_size,
            overlap=transcript.overlap,
            custom_prompt=custom_prompt
        )

        # Create final summary structure by aggregating chunk results
        final_summary = {
            "MeetingName": "",
            "People": {"title": "People", "blocks": []},
            "SessionSummary": {"title": "Session Summary", "blocks": []},
            "CriticalDeadlines": {"title": "Critical Deadlines", "blocks": []},
            "KeyItemsDecisions": {"title": "Key Items & Decisions", "blocks": []},
            "ImmediateActionItems": {"title": "Immediate Action Items", "blocks": []},
            "NextSteps": {"title": "Next Steps", "blocks": []},
            # "OtherImportantPoints": {"title": "Other Important Points", "blocks": []},
            # "ClosingRemarks": {"title": "Closing Remarks", "blocks": []},
            "MeetingNotes": {
                "meeting_name": "",
                "sections": []
            }
        }

        # Process each chunk's data
        for json_str in all_json_data:
            try:
                json_dict = json.loads(json_str)
                if "MeetingName" in json_dict and json_dict["MeetingName"]:
                    final_summary["MeetingName"] = json_dict["MeetingName"]
                for key in final_summary:
                    if key == "MeetingNotes" and key in json_dict:
                        # Handle MeetingNotes sections
                        if isinstance(json_dict[key].get("sections"), list):
                            # Ensure each section has blocks array
                            for section in json_dict[key]["sections"]:
                                if not section.get("blocks"):
                                    section["blocks"] = []
                            final_summary[key]["sections"].extend(json_dict[key]["sections"])
                        if json_dict[key].get("meeting_name"):
                            final_summary[key]["meeting_name"] = json_dict[key]["meeting_name"]
                    elif key != "MeetingName" and key in json_dict and isinstance(json_dict[key], dict) and "blocks" in json_dict[key]:
                        if isinstance(json_dict[key]["blocks"], list):
                            final_summary[key]["blocks"].extend(json_dict[key]["blocks"])
                            # Also add as a new section in MeetingNotes if not already present
                            section_exists = False
                            for section in final_summary["MeetingNotes"]["sections"]:
                                if section["title"] == json_dict[key]["title"]:
                                    section["blocks"].extend(json_dict[key]["blocks"])
                                    section_exists = True
                                    break
                            
                            if not section_exists:
                                final_summary["MeetingNotes"]["sections"].append({
                                    "title": json_dict[key]["title"],
                                    "blocks": json_dict[key]["blocks"].copy() if json_dict[key]["blocks"] else []
                                })
            except json.JSONDecodeError as e:
                logger.error(f"Failed to parse JSON chunk for {process_id}: {e}. Chunk: {json_str[:100]}...")
            except Exception as e:
                logger.error(f"Error processing chunk data for {process_id}: {e}. Chunk: {json_str[:100]}...")

        # Update database with meeting name using meeting_id
        if final_summary["MeetingName"]:
            await processor.db.update_meeting_name(transcript.meeting_id, final_summary["MeetingName"])

        # Save final result
        if all_json_data:
            await processor.db.update_process(process_id, status="completed", result=json.dumps(final_summary))
            logger.info(f"Background processing completed for process_id: {process_id}")
        else:
            error_msg = "Summary generation failed: No chunks were processed successfully. Check logs for specific errors."
            await processor.db.update_process(process_id, status="failed", error=error_msg)
            logger.error(f"Background processing failed for process_id: {process_id} - {error_msg}")

    except ValueError as e:
        # Handle specific value errors (like API key issues)
        error_msg = str(e)
        logger.error(f"Configuration error in background processing for {process_id}: {error_msg}", exc_info=True)
        try:
            await processor.db.update_process(process_id, status="failed", error=error_msg)
        except Exception as db_e:
            logger.error(f"Failed to update DB status to failed for {process_id}: {db_e}", exc_info=True)
    except Exception as e:
        # Handle all other exceptions
        error_msg = f"Processing error: {str(e)}"
        logger.error(f"Error in background processing for {process_id}: {error_msg}", exc_info=True)
        try:
            await processor.db.update_process(process_id, status="failed", error=error_msg)
        except Exception as db_e:
            logger.error(f"Failed to update DB status to failed for {process_id}: {db_e}", exc_info=True)

@app.post("/process-transcript")
async def process_transcript_api(
    transcript: TranscriptRequest,
    background_tasks: BackgroundTasks
):
    """Process a transcript text with background processing"""
    try:
        # Create new process linked to meeting_id
        process_id = await processor.db.create_process(transcript.meeting_id)

        # Save transcript data associated with meeting_id
        await processor.db.save_transcript(
            transcript.meeting_id,
            transcript.text,
            transcript.model,
            transcript.model_name,
            transcript.chunk_size,
            transcript.overlap
        )

        custom_prompt = transcript.custom_prompt

        # Start background processing
        background_tasks.add_task(
            process_transcript_background,
            process_id,
            transcript,
            custom_prompt
        )

        return JSONResponse({
            "message": "Processing started",
            "process_id": process_id
        })

    except Exception as e:
        logger.error(f"Error in process_transcript_api: {str(e)}", exc_info=True)
        raise HTTPException(status_code=500, detail=str(e))

@app.get("/get-summary/{meeting_id}")
async def get_summary(meeting_id: str):
    """Get the summary for a given meeting ID"""
    try:
        result = await processor.db.get_transcript_data(meeting_id)
        if not result:
            return JSONResponse(
                status_code=404,
                content={
                    "status": "error",
                    "meetingName": None,
                    "meeting_id": meeting_id,
                    "data": None,
                    "start": None,
                    "end": None,
                    "error": "Meeting ID not found"
                }
            )

        status = result.get("status", "unknown").lower()
        logger.debug(f"Summary status for meeting {meeting_id}: {status}, error: {result.get('error')}")

        # Parse result data if available
        summary_data = None
        if result.get("result"):
            try:
                parsed_result = json.loads(result["result"])
                if isinstance(parsed_result, str):
                    summary_data = json.loads(parsed_result)
                else:
                    summary_data = parsed_result
                if not isinstance(summary_data, dict):
                    logger.error(f"Parsed summary data is not a dictionary for meeting {meeting_id}")
                    summary_data = None
            except json.JSONDecodeError as e:
                logger.error(f"Failed to parse JSON data for meeting {meeting_id}: {str(e)}")
                status = "failed"
                result["error"] = f"Invalid summary data format: {str(e)}"
            except Exception as e:
                logger.error(f"Unexpected error parsing summary data for {meeting_id}: {str(e)}")
                status = "failed"
                result["error"] = f"Error processing summary data: {str(e)}"

        # Transform summary data into frontend format if available - PRESERVE ORDER
        transformed_data = {}
        if isinstance(summary_data, dict) and status == "completed":
            # Add MeetingName to transformed data
            transformed_data["MeetingName"] = summary_data.get("MeetingName", "")

            # Map backend sections to frontend sections
            section_mapping = {
                # "SessionSummary": "key_points",
                # "ImmediateActionItems": "action_items",
                # "KeyItemsDecisions": "decisions",
                # "NextSteps": "next_steps",
                # "CriticalDeadlines": "critical_deadlines",
                # "People": "people"
            }

            # Add each section to transformed data
            for backend_key, frontend_key in section_mapping.items():
                if backend_key in summary_data and isinstance(summary_data[backend_key], dict):
                    transformed_data[frontend_key] = summary_data[backend_key]
            
            # Add meeting notes sections if available - PRESERVE ORDER AND HANDLE DUPLICATES
            if "MeetingNotes" in summary_data and isinstance(summary_data["MeetingNotes"], dict):
                meeting_notes = summary_data["MeetingNotes"]
                if isinstance(meeting_notes.get("sections"), list):
                    # Add section order array to maintain order
                    transformed_data["_section_order"] = []
                    used_keys = set()
                    
                    for index, section in enumerate(meeting_notes["sections"]):
                        if isinstance(section, dict) and "title" in section and "blocks" in section:
                            # Ensure blocks is a list to prevent frontend errors
                            if not isinstance(section.get("blocks"), list):
                                section["blocks"] = []
                                
                            # Convert title to snake_case key
                            base_key = section["title"].lower().replace(" & ", "_").replace(" ", "_")
                            
                            # Handle duplicate section names by adding index
                            key = base_key
                            if key in used_keys:
                                key = f"{base_key}_{index}"
                            
                            used_keys.add(key)
                            transformed_data[key] = section
                            # Only add to _section_order if the section was successfully added
                            transformed_data["_section_order"].append(key)

        response = {
            "status": "processing" if status in ["processing", "pending", "started"] else status,
            "meetingName": summary_data.get("MeetingName") if isinstance(summary_data, dict) else None,
            "meeting_id": meeting_id,
            "start": result.get("start_time"),
            "end": result.get("end_time"),
            "data": transformed_data if status == "completed" else None
        }

        if status == "failed":
            response["status"] = "error"
            response["error"] = result.get("error", "Unknown processing error")
            response["data"] = None
            response["meetingName"] = None
            logger.info(f"Returning failed status with error: {response['error']}")
            return JSONResponse(status_code=400, content=response)

        elif status in ["processing", "pending", "started"]:
            response["data"] = None
            return JSONResponse(status_code=202, content=response)

        elif status == "completed":
            if not summary_data:
                response["status"] = "error"
                response["error"] = "Completed but summary data is missing or invalid"
                response["data"] = None
                response["meetingName"] = None
                return JSONResponse(status_code=500, content=response)
            return JSONResponse(status_code=200, content=response)

        else:
            response["status"] = "error"
            response["error"] = f"Unknown or unexpected status: {status}"
            response["data"] = None
            response["meetingName"] = None
            return JSONResponse(status_code=500, content=response)

    except Exception as e:
        logger.error(f"Error getting summary for {meeting_id}: {str(e)}", exc_info=True)
        return JSONResponse(
            status_code=500,
            content={
                "status": "error",
                "meetingName": None,
                "meeting_id": meeting_id,
                "data": None,
                "start": None,
                "end": None,
                "error": f"Internal server error: {str(e)}"
            }
        )

@app.post("/save-transcript")
async def save_transcript(request: SaveTranscriptRequest):
    """Save transcript segments for a meeting without processing"""
    try:
        logger.info(f"Received save-transcript request for meeting: {request.meeting_title}")
        logger.info(f"Number of transcripts to save: {len(request.transcripts)}")

        # Generate a unique meeting ID
        meeting_id = f"meeting-{int(time.time() * 1000)}"

        # Save the meeting
        await db.save_meeting(meeting_id, request.meeting_title)

        # Save each transcript segment
        for transcript in request.transcripts:
            await db.save_meeting_transcript(
                meeting_id=meeting_id,
                transcript=transcript.text,
                timestamp=transcript.timestamp,
                summary="",
                action_items="",
                key_points=""
            )

        logger.info("Transcripts saved successfully")
        return {"status": "success", "message": "Transcript saved successfully", "meeting_id": meeting_id}
    except Exception as e:
        logger.error(f"Error saving transcript: {str(e)}", exc_info=True)
        raise HTTPException(status_code=500, detail=str(e))

@app.get("/get-model-config")
async def get_model_config():
    """Get the current model configuration"""
    model_config = await db.get_model_config()
    if model_config:
        api_key = await db.get_api_key(model_config["provider"])
        if api_key != None:
            model_config["apiKey"] = api_key
    return model_config

@app.post("/save-model-config")
async def save_model_config(request: SaveModelConfigRequest):
    """Save the model configuration"""
    await db.save_model_config(request.provider, request.model, request.whisperModel)
    if request.apiKey != None:
        await db.save_api_key(request.apiKey, request.provider)
    return {"status": "success", "message": "Model configuration saved successfully"}  

@app.get("/get-transcript-config")
async def get_transcript_config():
    """Get the current transcript configuration"""
    transcript_config = await db.get_transcript_config()
    if transcript_config:
        transcript_api_key = await db.get_transcript_api_key(transcript_config["provider"])
        if transcript_api_key != None:
            transcript_config["apiKey"] = transcript_api_key
    return transcript_config

@app.post("/save-transcript-config")
async def save_transcript_config(request: SaveTranscriptConfigRequest):
    """Save the transcript configuration"""
    await db.save_transcript_config(request.provider, request.model)
    if request.apiKey != None:
        await db.save_transcript_api_key(request.apiKey, request.provider)
    return {"status": "success", "message": "Transcript configuration saved successfully"}

class GetApiKeyRequest(BaseModel):
    provider: str

@app.post("/get-api-key")
async def get_api_key(request: GetApiKeyRequest):
    try:
        return await db.get_api_key(request.provider)
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))

@app.post("/get-transcript-api-key")
async def get_transcript_api_key(request: GetApiKeyRequest):
    try:
        return await db.get_transcript_api_key(request.provider)
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))

class MeetingSummaryUpdate(BaseModel):
    meeting_id: str
    summary: dict

@app.post("/save-meeting-summary")
async def save_meeting_summary(data: MeetingSummaryUpdate):
    """Save a meeting summary"""
    try:
        await db.update_meeting_summary(data.meeting_id, data.summary)
        return {"message": "Meeting summary saved successfully"}
    except ValueError as ve:
        logger.error(f"Value error saving meeting summary: {str(ve)}")
        raise HTTPException(status_code=404, detail=str(ve))
    except Exception as e:
        logger.error(f"Error saving meeting summary: {str(e)}", exc_info=True)
        raise HTTPException(status_code=500, detail=str(e))

class SearchRequest(BaseModel):
    query: str

@app.post("/search-transcripts")
async def search_transcripts(request: SearchRequest):
    """Search through meeting transcripts for the given query"""
    try:
        results = await db.search_transcripts(request.query)
        return JSONResponse(content=results)
    except Exception as e:
        logger.error(f"Error searching transcripts: {str(e)}")
        raise HTTPException(status_code=500, detail=str(e))

# @app.websocket("/ws/{meeting_id}")
# async def websocket_endpoint(websocket: WebSocket, meeting_id: str):
#     """WebSocket endpoint for real-time communication"""
#     await ws_manager.connect(websocket, meeting_id)
    
#     try:
#         while True:
#             # Receive transcript chunks from client
#             data = await websocket.receive_json()
            
#             if data.get("type") == "transcript":
#                 # Process transcript with AI service
#                 transcript_chunk = data.get("text", "")
                
#                 if transcript_chunk:
#                     # Get AI assistance for the transcript chunk
#                     ai_response = await ai_service.process_transcript_chunk(
#                         meeting_id=meeting_id,
#                         transcript_chunk=transcript_chunk,
#                         include_context=data.get("include_context", True)
#                     )
                    
#                     if ai_response:
#                         # Send AI response back to client
#                         await websocket.send_json({
#                             "type": "ai_assistance",
#                             "data": ai_response,
#                             "timestamp": datetime.utcnow().isoformat()
#                         })
                        
#                         # Store AI response in database
#                         await db.save_ai_response(meeting_id, ai_response)
            
#             elif data.get("type") == "command":
#                 command = data.get("command")
                
#                 if command == "get_context":
#                     # Send current context
#                     context = ai_service.get_or_create_context(meeting_id)
#                     await websocket.send_json({
#                         "type": "context",
#                         "data": context.get_full_context(),
#                         "timestamp": datetime.utcnow().isoformat()
#                     })
                
#                 elif command == "clear_context":
#                     # Clear context for meeting
#                     ai_service.clear_context(meeting_id)
#                     await websocket.send_json({
#                         "type": "context_cleared",
#                         "meeting_id": meeting_id,
#                         "timestamp": datetime.utcnow().isoformat()
#                     })
    
#     except WebSocketDisconnect:
#         ws_manager.disconnect(websocket, meeting_id)
#         logger.info(f"WebSocket disconnected for meeting {meeting_id}")
#     except Exception as e:
#         logger.error(f"WebSocket error for meeting {meeting_id}: {str(e)}", exc_info=True)
#         ws_manager.disconnect(websocket, meeting_id)

@app.post("/process-realtime-transcript")
async def process_realtime_transcript(request: RealtimeTranscriptRequest):
    """Process real-time transcript chunk and get AI assistance"""
    try:
        # Process transcript with AI service
        ai_response = await ai_service.process_transcript_chunk(
            meeting_id=request.meeting_id,
            transcript_chunk=request.transcript_chunk,
            include_context=request.include_context
        )
        
        if ai_response:
            # Broadcast to all WebSocket clients for this meeting
            # await ws_manager.send_to_meeting(request.meeting_id, {
            #     "type": "ai_assistance",
            #     "data": ai_response,
            #     "timestamp": datetime.utcnow().isoformat()
            # })
            
            # Store in database
            # await db.save_ai_response(request.meeting_id, ai_response)
            
            return JSONResponse({
                "status": "success",
                "ai_response": ai_response
            })
        else:
            return JSONResponse(
                status_code=503,
                content={
                    "status": "error",
                    "message": "AI service unavailable"
                }
            )
    
    except Exception as e:
        logger.error(f"Error processing realtime transcript: {str(e)}", exc_info=True)
        raise HTTPException(status_code=500, detail=str(e))

@app.on_event("startup")
async def startup_event():
    """Initialize services on startup"""
    logger.info("Starting up API services...")
    try:
        await ai_service.initialize()
        logger.info("AI service initialized successfully")
    except Exception as e:
        logger.error(f"Failed to initialize AI service: {str(e)}", exc_info=True)

def get_google_credentials():
    """Get Google credentials using OAuth2 (personal account) or service account"""
    # Scopes needed
    SCOPES = [
        'https://www.googleapis.com/auth/documents',
        'https://www.googleapis.com/auth/drive',
        'https://www.googleapis.com/auth/drive.file'
    ]
    
    # Try OAuth2 first (easier for testing)
    oauth_credentials_path = os.getenv('GOOGLE_OAUTH_CREDENTIALS_PATH')
    token_file = 'token.pickle'
    
    if oauth_credentials_path and os.path.exists(oauth_credentials_path):
        logger.info("Using OAuth2 credentials (personal account)")
        creds = None
        
        # Check if we have saved credentials
        if os.path.exists(token_file):
            with open(token_file, 'rb') as token:
                creds = pickle.load(token)
        
        # If no valid credentials, get new ones
        if not creds or not creds.valid:
            if creds and creds.expired and creds.refresh_token:
                creds.refresh(Request())
            else:
                flow = InstalledAppFlow.from_client_secrets_file(oauth_credentials_path, SCOPES)
                creds = flow.run_local_server(port=0)
            
            # Save credentials for future use
            with open(token_file, 'wb') as token:
                pickle.dump(creds, token)
        
        return creds
    
    # Fallback to service account
    service_account_path = os.getenv('GOOGLE_SERVICE_ACCOUNT_PATH')
    if service_account_path and os.path.exists(service_account_path):
        logger.info("Using service account credentials")
        return service_account.Credentials.from_service_account_file(service_account_path, scopes=SCOPES)
    
    return None

def process_audio_with_genai(audio_content: bytes, filename: str) -> str:
    """Process audio file with Google GenAI and return transcript"""
    try:
        # Get GenAI API key from environment
        genai_api_key = os.getenv('GENAI_API_KEY')
        if not genai_api_key:
            logger.warning("GENAI_API_KEY not found in environment variables")
            return "GenAI processing skipped: No API key configured"
        
        # Initialize GenAI client
        client = genai.Client(api_key=genai_api_key)
        logger.info("GenAI client initialized")
        
        # Create temporary file for the audio
        with tempfile.NamedTemporaryFile(suffix='.wav', delete=False) as temp_file:
            temp_file.write(audio_content)
            temp_file_path = temp_file.name
        
        try:
            # Upload file to GenAI
            logger.info(f"Uploading audio file to GenAI: {filename}")
            myfile = client.files.upload(file=temp_file_path)
            logger.info(f"File uploaded to GenAI successfully")
            
            # Generate content with custom prompt (similar to temp.py)
            response = client.models.generate_content(
                model="gemini-2.5-flash", 
                contents=[
                    "Generate a transcript of the speech. In the transcript, whenever someone says anything related to money, add this line: 'Gleany Glean'. Label different speakers as 'Speaker 1' and 'Speaker 2', extra custom text added in transcript should be labeled as 'Speaker Glean'.", 
                    myfile
                ]
            )
            
            logger.info("GenAI processing completed successfully")
            return response.text
            
        finally:
            # Clean up temporary file
            if os.path.exists(temp_file_path):
                os.unlink(temp_file_path)
                logger.debug("Temporary audio file cleaned up")
    
    except Exception as e:
        logger.error(f"Error processing audio with GenAI: {str(e)}")
        return f"GenAI processing failed: {str(e)}"

def create_google_doc(title: str, content: str):
    """Create a Google Doc with the specified title and content"""
    
    # Simple fallback mode - just log the data instead of creating actual docs
    if os.getenv('GOOGLE_DOCS_TEST_MODE') == 'true':
        logger.info("=== GOOGLE DOCS TEST MODE ===")
        logger.info(f"Would create Google Doc:")
        logger.info(f"Title: {title}")
        logger.info(f"Content: {content}")
        logger.info("=============================")
        return {
            "document_id": "test-mode-doc-id",
            "url": "https://docs.google.com/test-mode",
            "title": title,
            "test_mode": True
        }
    
    try:
        # Get credentials
        credentials = get_google_credentials()
        if not credentials:
            logger.warning("No Google credentials found. Set GOOGLE_DOCS_TEST_MODE=true for testing.")
            return None
        
        # Build services
        docs_service = build('docs', 'v1', credentials=credentials)
        drive_service = build('drive', 'v3', credentials=credentials)
        
        # Create a new document
        document = {
            'title': title
        }
        
        doc = docs_service.documents().create(body=document).execute()
        document_id = doc.get('documentId')
        
        logger.info(f"Created Google Doc with ID: {document_id}")
        
        # Add content to the document
        requests = [
            {
                'insertText': {
                    'location': {
                        'index': 1,
                    },
                    'text': content
                }
            }
        ]
        
        docs_service.documents().batchUpdate(
            documentId=document_id,
            body={'requests': requests}
        ).execute()
        
        # Make the document public (optional)
        try:
            drive_service.permissions().create(
                fileId=document_id,
                body={'role': 'reader', 'type': 'anyone'}
            ).execute()
            logger.info("Made document publicly readable")
        except Exception as perm_error:
            logger.warning(f"Could not make document public: {perm_error}")
        
        doc_url = f"https://docs.google.com/document/d/{document_id}/edit"
        logger.info(f"Google Doc created successfully: {doc_url}")
        
        return {
            "document_id": document_id,
            "url": doc_url,
            "title": title
        }
        
    except Exception as e:
        logger.error(f"Failed to create Google Doc: {str(e)}")
        return None

@app.post("/upload-audio")
async def upload_audio(file: UploadFile = File(...)):
    """Upload audio file endpoint"""
    try:
        logger.info(f"Received audio file upload: {file.filename}")
        logger.info(f"File size: {file.size} bytes")
        logger.info(f"Content type: {file.content_type}")
        
        # Validate file type (optional - you can add more validation)
        allowed_types = ["audio/wav", "audio/mp3", "audio/mpeg", "audio/x-wav", "application/octet-stream"]
        if file.content_type not in allowed_types:
            logger.warning(f"Unsupported file type: {file.content_type}")
        
        # Read file content (you can process it here if needed)
        content = await file.read()
        logger.info(f"Successfully read {len(content)} bytes from audio file")
        
        # Process audio with GenAI to get transcript
        logger.info("Processing audio with GenAI...")
        genai_transcript = process_audio_with_genai(content, file.filename or "recording.wav")
        
        # Create Google Doc with GenAI transcript as content
        doc_title = file.filename or "Audio Recording"
        doc_content = f"""Audio Transcription

File: {file.filename}
Size: {len(content)} bytes
Content Type: {file.content_type}
Processed at: {time.strftime('%Y-%m-%d %H:%M:%S')}

--- TRANSCRIPT ---
{genai_transcript}
"""
        
        google_doc_result = create_google_doc(doc_title, doc_content)
        
        response_data = {
            "success": True,
            "message": "Audio file uploaded and processed successfully",
            "filename": file.filename,
            "size": len(content),
            "content_type": file.content_type,
            "transcript": genai_transcript[:500] + "..." if len(genai_transcript) > 500 else genai_transcript  # Include first 500 chars in response
        }
        
        # Add Google Doc info to response if creation was successful
        if google_doc_result:
            response_data["google_doc"] = google_doc_result
            logger.info(f"Google Doc created for audio file with transcript: {google_doc_result['url']}")
        else:
            response_data["google_doc"] = None
            logger.info("Google Doc creation skipped or failed")
        
        return response_data
        
    except Exception as e:
        logger.error(f"Error uploading audio file: {str(e)}", exc_info=True)
        raise HTTPException(status_code=500, detail=f"Failed to upload audio file: {str(e)}")

@app.on_event("shutdown")
async def shutdown_event():
    """Cleanup on API shutdown"""
    logger.info("API shutting down, cleaning up resources")
    try:
        processor.cleanup()
        await cleanup_ai_service()
        logger.info("Successfully cleaned up resources")
    except Exception as e:
        logger.error(f"Error during cleanup: {str(e)}", exc_info=True)

if __name__ == "__main__":
    import multiprocessing
    multiprocessing.freeze_support()
    uvicorn.run("main:app", host="0.0.0.0", port=5167, reload=True)
