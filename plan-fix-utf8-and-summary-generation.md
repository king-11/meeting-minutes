# Simplified Fix Plan: UTF-8 Panic & Summary Generation Failures

## Executive Summary
This plan addresses two critical production issues in the Meeting Minutes application:
1. **Critical**: UTF-8 boundary panic causing application crashes
2. **High Priority**: Summary generation failures with unhelpful error messages

---

## Issue 1: UTF-8 Boundary Panic (CRITICAL)

### Problem
**Error Location**: `frontend/src-tauri/src/api.rs:283`

**Stack Trace**:
```
thread 'tokio-runtime-worker' panicked at src/api.rs:283:50:
byte index 200 is not a char boundary; it is inside '♪' (bytes 198..201)
```

**Root Cause**: 
- String slicing at byte position without UTF-8 boundary checking
- Multi-byte characters (like '♪') can span multiple bytes
- Slicing at byte 200 cuts through the character causing panic

### Solution

#### File: `frontend/src-tauri/src/api.rs` (Line 283)

**Current Code**:
```rust
log_info!("Response body: {}", &response_text[..std::cmp::min(200, response_text.len())]);
```

**Fixed Code**:
```rust
// Safe UTF-8 aware truncation for logging
let log_preview = if response_text.len() <= 200 {
    response_text.as_str()
} else {
    // Find the last valid character boundary at or before position 200
    let mut boundary = 200;
    while !response_text.is_char_boundary(boundary) && boundary > 0 {
        boundary -= 1;
    }
    &response_text[..boundary]
};
log_info!("Response body preview ({}B): {}", log_preview.len(), log_preview);
```

---

## Issue 2: Summary Generation Failures

### Problem
**Error Location**: `backend/app/main.py:370`

**Current Error**: 
```
"Summary generation failed: No chunks were processed successfully. Check logs for specific errors."
```

**Root Causes**:
- No validation of transcript content before processing
- Generic error messages that don't explain the actual problem
- Silent failures when AI services are unavailable

### Solution

#### File: `backend/app/main.py`

**1. Add Input Validation** (Insert after line 283 in `process_transcript_background`):
```python
async def process_transcript_background(process_id: str, transcript: TranscriptRequest, custom_prompt: str):
    """Background task to process transcript"""
    try:
        logger.info(f"Starting background processing for process_id: {process_id}")
        
        # Early validation for common issues
        if not transcript.text or len(transcript.text.strip()) < 10:
            error_msg = f"Transcript too short ({len(transcript.text.strip())} chars, minimum 10)"
            logger.warning(f"Validation failed for {process_id}: {error_msg}")
            await processor.db.update_process(process_id, status="failed", error=error_msg)
            return
            
        if not transcript.model or not transcript.model_name:
            error_msg = "AI model not specified"
            await processor.db.update_process(process_id, status="failed", error=error_msg)
            return
        
        # Log processing details for debugging
        logger.info(f"Processing {process_id}: {len(transcript.text)} chars with {transcript.model}/{transcript.model_name}")
        
        # Continue with existing processing...
```

**2. Enhance Error Messages** (Update lines 369-372):
```python
if all_json_data:
    # Save final summary
    await processor.db.update_process(process_id, status="completed", result=json.dumps(final_summary))
    logger.info(f"Background processing completed for {process_id}: {len(all_json_data)} chunks")
else:
    # Determine specific failure reason
    if chunks_processed == 0:
        error_msg = "No chunks were created from transcript (text may be too short)"
    elif transcript.model == "ollama":
        error_msg = f"Ollama processing failed - ensure Ollama is running and {transcript.model_name} model is available"
    elif transcript.model in ["gemini", "claude", "groq"]:
        error_msg = f"{transcript.model.title()} API processing failed - check API key configuration"
    else:
        error_msg = f"All {chunks_processed} chunks failed to process with {transcript.model}"
    
    await processor.db.update_process(process_id, status="failed", error=error_msg)
    logger.error(f"Background processing failed for {process_id}: {error_msg}")
```

#### File: `backend/app/transcript_processor.py`

**Add Basic Validation** (Update line 87):
```python
async def process_transcript(self, text: str, model: str, model_name: str, 
                            chunk_size: int = 5000, overlap: int = 1000, 
                            custom_prompt: str = "") -> Tuple[int, List[str]]:
    """Process transcript with validation and detailed logging"""
    
    # Basic validation
    if not text or len(text.strip()) < 10:
        raise ValueError(f"Transcript too short: {len(text.strip())} characters")
    
    if model == "ollama":
        # Quick check if Ollama is reachable
        import requests
        try:
            response = requests.get("http://localhost:11434/api/tags", timeout=2)
            if response.status_code != 200:
                raise ValueError(f"Ollama server not responding (status {response.status_code})")
        except requests.ConnectionError:
            raise ValueError("Cannot connect to Ollama - ensure it's running with 'ollama serve'")
        except requests.Timeout:
            raise ValueError("Ollama server timeout - it may be overloaded")
    
    logger.info(f"Processing transcript: {len(text)} chars with {model}/{model_name}")
    
    # Continue with existing processing...
```

---

## Testing

### Test UTF-8 Fix
```bash
# Create test with multi-byte characters
curl -X POST http://localhost:5167/test-endpoint \
  -H "Content-Type: application/json" \
  -d '{"text": "♪♪♪ Meeting notes 会議 📝 Very long text..."}'
```

### Test Summary Generation
```bash
# Test with empty transcript
curl -X POST http://localhost:5167/process-transcript \
  -d '{"text": "", "model": "ollama", "model_name": "llama2"}'

# Test with Ollama not running
systemctl stop ollama
curl -X POST http://localhost:5167/process-transcript \
  -d '{"text": "Meeting transcript...", "model": "ollama", "model_name": "llama2"}'
```

---

## Implementation Order

1. **Day 1 - Critical Fix**
   - Apply UTF-8 boundary fix to `api.rs`
   - Test with various UTF-8 characters
   - Deploy and monitor for panics

2. **Day 2 - Error Handling**
   - Add validation to `process_transcript_background`
   - Improve error messages
   - Add basic Ollama connectivity check
   - Test with various failure scenarios

---

## Success Metrics

- Zero UTF-8 panic errors in logs
- Clear, actionable error messages for users
- 90%+ reduction in generic "No chunks processed" errors
- Users understand why summary generation failed

---

## Notes

- No database migrations needed
- No complex health check systems
- No new UI components required
- Focus on fixing the core issues with minimal changes