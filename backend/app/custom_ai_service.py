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
            glean_request = {
                "saveChat": True,
                "agentId": self.agent_id,
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
                        
            # Call Glean API
            logger.info(f"Calling Glean API for meeting {meeting_id} with chunk: {transcript_chunk[:100]}...")
            
            async with self.session.post(
                f"{self.api_url}/chat?timezoneOffset=+330#stream",
                json=glean_request
            ) as response:
                logger.info(f"Glean API response status: {response.status}")
                
                if response.status == 200:
                    result = await response.json()
                    
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
                    
                    # Store AI response in context
                    # context.add_ai_response(ai_response, transcript_chunk)
                    
                    # return {
                    #     "meeting_id": meeting_id,
                    #     "response": ai_response,
                    #     "context": context.get_full_context(),
                    #     "suggestions": result.get("suggestions", []),
                    #     "relevant_documents": result.get("documents", []),
                    #     "timestamp": datetime.utcnow().isoformat(),
                    #     "raw_response": result  # Include raw response for debugging
                    # }
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
    
    async def get_summary_context(self, meeting_id: str) -> Optional[Dict[str, Any]]:
        """
        Get summary and context for a meeting using Glean AI
        
        Args:
            meeting_id: Unique identifier for the meeting
            
        Returns:
            Dict containing meeting context and suggestions
        """
        if meeting_id not in self.contexts:
            return None
            
        context = self.contexts[meeting_id]
        full_context = context.get_full_context()
        
        if not self.session:
            await self.initialize()
        
        try:
            # Build summary request for Glean
            summary_prompt = f"""Please provide a comprehensive summary of this meeting:

Meeting Context:
- Recent Transcript: {full_context.get('recent_transcript', 'No transcript available')}
- Current Topic: {full_context.get('current_topic', 'Not identified')}
- Participants: {', '.join(full_context.get('participants', [])) if full_context.get('participants') else 'Not identified'}

Please include:
1. Main topics discussed
2. Key decisions made
3. Action items
4. Important insights or concerns raised"""

            glean_request = {
                "saveChat": True,
                "agentId": self.agent_id,
                "messages": [
                    {
                        "agentConfig": {
                            "agent": "DEFAULT",
                            "mode": "DEFAULT"
                        },
                        "messageType": "CONTENT",
                        "author": "USER",
                        "fragments": [
                            {"text": summary_prompt}
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
            logger.info(f"Glean request: {glean_request}")
            # Request comprehensive summary from Glean
            async with self.session.post(
                f"{self.api_url}/chat?timezoneOffset=+330#stream",
                json=glean_request
            ) as response:
                if response.status == 200:
                    result = await response.json()
                    logger.info(f"Glean result: {result}")
                    # Extract the summary from Glean response
                    summary = ""
                    if "messages" in result and len(result["messages"]) > 0:
                        for message in result["messages"]:
                            # Glean API may return 'GLEAN_AI' or 'ASSISTANT' as the author
                            if message.get("author") in ["GLEAN_AI", "ASSISTANT"] and "fragments" in message:
                                for fragment in message["fragments"]:
                                    if "text" in fragment:
                                        summary += fragment["text"]
                    logger.info(f"Glean summary: {summary}")
                    
                    return {
                        "meeting_id": meeting_id,
                        "summary": summary,
                        "context": full_context,
                        "timestamp": datetime.utcnow().isoformat()
                    }
                else:
                    logger.error(f"Failed to get summary for meeting {meeting_id}")
                    return None
                    
        except Exception as e:
            logger.error(f"Error getting summary for meeting {meeting_id}: {str(e)}", exc_info=True)
            return None
    
    def clear_context(self, meeting_id: str):
        """Clear context for a specific meeting"""
        if meeting_id in self.contexts:
            self.contexts[meeting_id].clear()
            del self.contexts[meeting_id]
    
    async def search_knowledge_base(self, query: str, meeting_id: Optional[str] = None) -> List[Dict]:
        """
        Search the Glean knowledge base for relevant information
        
        Args:
            query: Search query
            meeting_id: Optional meeting ID for context
            
        Returns:
            List of relevant documents/information
        """
        if not self.session:
            await self.initialize()
        
        try:
            # Build search query with optional context
            search_prompt = query
            if meeting_id and meeting_id in self.contexts:
                context = self.contexts[meeting_id].get_recent_context()
                if context:
                    search_prompt = f"In the context of this meeting discussion: {context}\n\nFind information about: {query}"
            
            glean_request = {
                "saveChat": True,
                "agentId": self.agent_id,
                "messages": [
                    {
                        "agentConfig": {
                            "agent": "DEFAULT",
                            "mode": "DEFAULT"
                        },
                        "messageType": "CONTENT",
                        "author": "USER",
                        "fragments": [
                            {"text": search_prompt}
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
            
            async with self.session.post(
                f"{self.api_url}/chat?timezoneOffset=+330#stream",
                json=glean_request
            ) as response:
                if response.status == 200:
                    result = await response.json()
                    
                    # Extract search results from Glean response
                    search_results = []
                    
                    # Extract text response
                    response_text = ""
                    if "messages" in result and len(result["messages"]) > 0:
                        for message in result["messages"]:
                            # Glean API may return 'GLEAN_AI' or 'ASSISTANT' as the author
                            if message.get("author") in ["GLEAN_AI", "ASSISTANT"] and "fragments" in message:
                                for fragment in message["fragments"]:
                                    if "text" in fragment:
                                        response_text += fragment["text"]
                    
                    # Create search result entry
                    if response_text:
                        search_results.append({
                            "content": response_text,
                            "query": query,
                            "timestamp": datetime.utcnow().isoformat()
                        })
                    
                    # Add any documents referenced in the response
                    if "documents" in result:
                        for doc in result["documents"]:
                            search_results.append({
                                "title": doc.get("title", "Untitled"),
                                "content": doc.get("content", ""),
                                "url": doc.get("url", ""),
                                "type": "document"
                            })
                    
                    return search_results
                else:
                    logger.error(f"Search failed with status {response.status}")
                    return []
                    
        except Exception as e:
            logger.error(f"Error searching knowledge base: {str(e)}", exc_info=True)
            return []


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
