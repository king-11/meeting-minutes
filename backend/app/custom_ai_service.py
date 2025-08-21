"""
Custom AI Service Integration Module
Handles communication with Glean chat AI tool for real-time contextual assistance
"""

import aiohttp
import json
import logging
from typing import Dict, Optional, List, Any
from datetime import datetime, timedelta
import asyncio
from collections import deque

logger = logging.getLogger(__name__)

class ConversationContext:
    """Manages conversation context for real-time AI assistance"""
    
    def __init__(self, max_history_items: int = 50, context_window_minutes: int = 5):
        self.transcript_history = deque(maxlen=max_history_items)
        self.ai_responses = deque(maxlen=max_history_items)
        self.context_window = timedelta(minutes=context_window_minutes)
        self.last_update = datetime.utcnow()
        self.current_topic = None
        self.participants = set()
        
    def add_transcript(self, text: str, timestamp: Optional[datetime] = None):
        """Add new transcript chunk to context"""
        if timestamp is None:
            timestamp = datetime.utcnow()
        
        self.transcript_history.append({
            "text": text,
            "timestamp": timestamp.isoformat()
        })
        self.last_update = timestamp
        
    def add_ai_response(self, response: str, query: str, timestamp: Optional[datetime] = None):
        """Store AI response for context continuity"""
        if timestamp is None:
            timestamp = datetime.utcnow()
            
        self.ai_responses.append({
            "query": query,
            "response": response,
            "timestamp": timestamp.isoformat()
        })
        
    def get_recent_context(self) -> str:
        """Get recent conversation context within the time window"""
        cutoff_time = datetime.utcnow() - self.context_window
        
        recent_transcripts = []
        for item in reversed(self.transcript_history):
            item_time = datetime.fromisoformat(item["timestamp"])
            if item_time >= cutoff_time:
                recent_transcripts.append(item["text"])
            else:
                break
                
        return " ".join(reversed(recent_transcripts))
    
    def get_full_context(self) -> Dict[str, Any]:
        """Get complete context for AI query"""
        return {
            "recent_transcript": self.get_recent_context(),
            "current_topic": self.current_topic,
            "participants": list(self.participants),
            "last_responses": list(self.ai_responses)[-3:] if self.ai_responses else []
        }
    
    def clear(self):
        """Clear conversation context"""
        self.transcript_history.clear()
        self.ai_responses.clear()
        self.current_topic = None
        self.participants.clear()


class CustomAIService:
    """Service for integrating with Glean chat AI tool"""
    
    def __init__(self, api_url: str = None, api_key: Optional[str] = None, timeout: int = 30):
        """
        Initialize the Custom AI Service with Glean integration
        
        Args:
            api_url: Base URL for the Glean API (defaults to Glean prod endpoint)
            api_key: Optional API key for authentication
            timeout: Request timeout in seconds
        """
        # Default to Glean API if not specified
        self.api_url = api_url if api_url else "https://scio-prod-be.glean.com/rest/api/v1"
        self.api_key = api_key
        self.agent_id = "1d14f9592fe34b288dec869a218d44de"
        self.timeout = aiohttp.ClientTimeout(total=timeout)
        self.session = None
        self.contexts = {}  # Store contexts per meeting_id
        self.chat_ids = {}  # Store chatId per meeting_id for subsequent API calls
        
    async def initialize(self):
        """Initialize the aiohttp session with Glean API headers"""
        if not self.session:
            headers = {
                "Content-Type": "application/json",
                "Accept": "application/json",
                "Authorization": f"Bearer {self.api_key}",
            }
            
            self.session = aiohttp.ClientSession(
                headers=headers,
                timeout=self.timeout
            )
    
    async def close(self):
        """Close the aiohttp session"""
        if self.session:
            await self.session.close()
            self.session = None
    
    def get_or_create_context(self, meeting_id: str) -> ConversationContext:
        """Get or create a conversation context for a meeting"""
        if meeting_id not in self.contexts:
            self.contexts[meeting_id] = ConversationContext()
        return self.contexts[meeting_id]
    
    async def process_transcript_chunk(
        self, 
        meeting_id: str, 
        transcript_chunk: str,
        include_context: bool = True
    ) -> Optional[Dict[str, Any]]:
        """
        Process a real-time transcript chunk using Glean AI
        
        Args:
            meeting_id: Unique identifier for the meeting
            transcript_chunk: New transcript text to process
            include_context: Whether to include previous context
            
        Returns:
            Dict containing AI response and metadata, or None if failed
        """
        if not self.session:
            await self.initialize()
        
        try:
            if "INAUDIBLE" in transcript_chunk.upper():
                return None
            if transcript_chunk.startswith("(") and transcript_chunk.endswith(")") or transcript_chunk.startswith("[") and transcript_chunk.endswith("]"):
                return None
            
            # Get or create context for this meeting
            context = self.get_or_create_context(meeting_id)
            
            # Add new transcript to context
            context.add_transcript(transcript_chunk)
            
            # Build the question with context if needed
            question = transcript_chunk
            if include_context:
                recent_context = context.get_recent_context()
                if recent_context and recent_context != transcript_chunk:
                    # question = f"Based on this meeting context: {recent_context}\n\nProvide insights about: {transcript_chunk}"
                    question = transcript_chunk

            # Prepare Glean API request
            # Use chatId if we have it for this meeting, otherwise use agentId
            glean_request = {
                "saveChat": True,
                "messages": [
                    {
                        "agentConfig": {
                            "agent": "DEFAULT",
                            "mode": "DEFAULT"
                        },
                        "messageType": "CONTENT",
                        "author": "USER",
                        "fragments": [
                            {"text": question}
                        ]
                    }
                ],
                "agentConfig": {
                    "agent": "DEFAULT",
                    "mode": "DEFAULT"
                },
                "timeoutMillis": 30000,
                "stream": False
            }
            
            # Add chatId or agentId based on whether this is a continuation
            if meeting_id in self.chat_ids:
                glean_request["chatId"] = self.chat_ids[meeting_id]
                logger.info(f"Using existing chatId for meeting {meeting_id}: {self.chat_ids[meeting_id]}")
            else:
                glean_request["agentId"] = self.agent_id
                logger.info(f"Using agentId for first call of meeting {meeting_id}")
                        
            # Call Glean API
            logger.info(f"Calling Glean API for meeting {meeting_id} with chunk: {transcript_chunk[:100]}...")
            
            async with self.session.post(
                f"{self.api_url}/chat?timezoneOffset=+330#stream",
                json=glean_request
            ) as response:
                logger.info(f"Glean API response status: {response.status}")
                
                if response.status == 200:
                    result = await response.json()
                    
                    # Extract and store chatId if present (first call for this meeting)
                    if "chatId" in result:
                        self.chat_ids[meeting_id] = result["chatId"]
                        logger.info(f"Stored chatId for meeting {meeting_id}: {result['chatId']}")
                    
                    # Extract the AI response from Glean format
                    ai_response = ""
                    if "messages" in result and len(result["messages"]) > 0:
                        for message in result["messages"]:
                            # Glean API may return 'GLEAN_AI' or 'ASSISTANT' as the author
                            if message.get("author") in ["GLEAN_AI", "ASSISTANT"] and "fragments" in message:
                                for fragment in message["fragments"]:
                                    if "action" in fragment:
                                        continue
                                    if "text" in fragment:
                                        ai_response = fragment["text"]
                    
                    logger.info(f"Extracted AI response: {ai_response[:200]}..." if ai_response else "No AI response extracted")
                    return "Glean AI Response: "+ai_response
                else:
                    error_text = await response.text()
                    logger.error(f"Glean API error (status {response.status}): {error_text}")
                    return None
                    
        except asyncio.TimeoutError:
            logger.error(f"Timeout calling AI tool for meeting {meeting_id}")
            return None
        except Exception as e:
            logger.error(f"Error processing transcript chunk for meeting {meeting_id}: {str(e)}", exc_info=True)
            return None

# Singleton instance
_ai_service_instance = None

def get_ai_service(api_url: Optional[str] = None, api_key: Optional[str] = None) -> CustomAIService:
    """Get or create the Glean AI service singleton instance"""
    global _ai_service_instance
    
    if _ai_service_instance is None:
        import os
        # Get from environment if not provided, default to Glean API
        if api_url is None:
            api_url = os.getenv("GLEAN_API_URL", "https://scio-prod-be.glean.com/rest/api/v1")
        if api_key is None:
            api_key = os.getenv("GLEAN_API_KEY")
        
        _ai_service_instance = CustomAIService(api_url, api_key)
    
    return _ai_service_instance


async def cleanup_ai_service():
    """Cleanup the AI service singleton"""
    global _ai_service_instance
    if _ai_service_instance:
        await _ai_service_instance.close()
        _ai_service_instance = None
