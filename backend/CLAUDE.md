# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a **meeting transcription and summarization backend** that combines:
- **whisper.cpp server** - C++ implementation for fast audio transcription
- **FastAPI application** - Python backend for meeting management and AI processing
- **Multiple deployment options** - Native, Docker, and pre-built releases

## Architecture

### Core Components

1. **Whisper Server (Port 8178)**
   - Custom C++ server built from whisper.cpp submodule
   - Located in `whisper-custom/server/` with modifications
   - Handles real-time audio transcription via HTTP/WebSocket
   - Supports multiple model sizes (tiny to large-v3)

2. **FastAPI Backend (Port 5167)**
   - Main application in `app/main.py`
   - Meeting management with SQLite database (`meeting_minutes.db`)
   - AI summarization using multiple providers (Claude, Groq, Ollama)
   - WebSocket support for real-time updates
   - API documentation at `/docs`

3. **Key Python Modules**
   - `app/transcript_processor.py` - Pydantic-AI based transcript processing
   - `app/custom_ai_service.py` - AI service factory for multiple providers
   - `app/db.py` - Database management layer
   - `app/schema_validator.py` - Request/response validation

## Development Commands

### Building and Running (Native)

**macOS/Linux:**
```bash
# Build whisper server with model
./build_whisper.sh small

# Start both services
./clean_start_backend.sh

# Alternative: Download model only
./download-ggml-model.sh base.en
```

**Windows:**
```cmd
# Build whisper server
build_whisper.cmd small

# Start services interactively
start_with_output.ps1

# Alternative: Clean start
clean_start_backend.cmd
```

### Docker Deployment

**Build and Run:**
```bash
# Build images
./build-docker.sh cpu        # or: .\build-docker.ps1 cpu
./build-docker.sh gpu        # GPU support

# Interactive setup (recommended)
./run-docker.sh start --interactive   # or: .\run-docker.ps1 start -Interactive

# Quick start with defaults
./run-docker.sh start --detach       # or: .\run-docker.ps1 start -Detach

# Monitor services
./run-docker.sh logs --service whisper --follow
./run-docker.sh status
```

### Testing and Development

**No formal test suite exists** - testing is done manually by running services and checking endpoints:
- Whisper health: `curl http://localhost:8178/`
- Backend health: `curl http://localhost:5167/get-meetings`
- API docs: http://localhost:5167/docs

**Linting/Type Checking:**
```bash
# Python linting (if ruff is installed)
ruff check app/

# No TypeScript/JavaScript linting configured
```

## Key Files and Patterns

### Configuration
- **Environment Variables**: `.env` file in app directory
- **Docker Configuration**: `docker-compose.yml` with service definitions
- **Model Storage**: `models/` directory for whisper models
- **Database**: SQLite at `data/meeting_minutes.db`

### Build Process Flow
1. Updates whisper.cpp git submodule
2. Copies custom server from `whisper-custom/server/`
3. Builds with CMake (native) or Docker
4. Creates Python venv and installs requirements
5. Downloads specified whisper model
6. Creates `whisper-server-package/` with binaries

### API Patterns
- FastAPI with Pydantic models for validation
- WebSocket support for real-time communication
- CORS enabled for all origins (development mode)
- Background tasks for async processing
- Multiple AI provider support via factory pattern

## Dependencies and Requirements

### Python (requirements.txt)
- pydantic-ai==0.2.15 - AI agent framework
- fastapi==0.115.9 - Web framework
- uvicorn==0.34.0 - ASGI server
- aiosqlite==0.21.0 - Async SQLite
- ollama==0.5.2 - Local LLM support
- google-genai==1.31.0 - Google AI integration

### System Requirements
- **Native Build**: CMake, C++ compiler, Python 3.8+
- **macOS**: Xcode tools, LLVM, libomp for optimization
- **Windows**: Visual Studio Build Tools, PowerShell 5.0+
- **Docker**: 8GB+ RAM allocation recommended

## Common Tasks

### Adding New AI Provider
1. Extend `app/custom_ai_service.py` factory
2. Add provider-specific model mapping
3. Update environment variables in `.env`

### Modifying Whisper Server
1. Edit files in `whisper-custom/server/`
2. Rebuild with `build_whisper.sh/cmd`
3. Custom modifications override whisper.cpp defaults

### Database Schema Changes
1. Modify schema in `app/db.py`
2. No formal migration system - manual updates required
3. Backup existing database before changes

### Debugging Audio Processing
- Check logs for "Dropped old audio chunk" warnings
- Indicates insufficient resources for Docker containers
- Allocate more RAM/CPU or use native deployment

## Port Configuration
- **8178**: Whisper transcription server
- **5167**: FastAPI backend application
- **11434**: Ollama (if using local LLM)

## Model Selection Guide
- **tiny/base**: Fast, lower accuracy, good for testing
- **small/medium**: Balanced performance and accuracy
- **large-v3**: Best accuracy, requires significant resources
- Append `.en` for English-only models (faster)