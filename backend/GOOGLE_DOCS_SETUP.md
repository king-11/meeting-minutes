# Google Docs Integration Setup

The upload-audio endpoint now creates a Google Doc with the audio file information. 

## 🚀 Quick Test Mode (No Setup Required!)

For **immediate testing**, add this to your `.env` file:
```
GOOGLE_DOCS_TEST_MODE=true
```

This will log what would be created without actually creating Google Docs. Perfect for testing!

## 📝 Easy Setup with Personal Account (Recommended)

### Option A: Use Your Personal Google Account (OAuth2)
**Much easier than service accounts!**

1. Go to [Google Cloud Console](https://console.cloud.google.com/)
2. Create a new project
3. Enable APIs: Google Docs API + Google Drive API
4. Go to "Credentials" → "Create Credentials" → "OAuth 2.0 Client IDs"
5. Application type: "Desktop application"
6. Download the JSON file
7. Add to `.env`: `GOOGLE_OAUTH_CREDENTIALS_PATH=/path/to/credentials.json`

**That's it!** The first time you upload, it will open a browser for Google login.

## 🔧 Advanced Setup with Service Account

### Option B: Service Account (For Production)

1. **Create Service Account:**
   - Go to "IAM & Admin" > "Service Accounts"
   - Click "Create Service Account"
   - Name: "meeting-minutes-docs"
   - Grant role: **Editor**

2. **Generate Key File:**
   - Click on service account → "Keys" tab
   - "Add Key" → "Create New Key" → "JSON"
   - Download the JSON file

3. **Set Environment Variable:**
   ```
   GOOGLE_SERVICE_ACCOUNT_PATH=/path/to/service-account-key.json
   ```

## 🎯 Summary - Choose Your Method

### For Immediate Testing (No Google setup):
```bash
# Add to .env file
GOOGLE_DOCS_TEST_MODE=true
```

### For Personal Use (Easy setup):
```bash
# Add to .env file  
GOOGLE_OAUTH_CREDENTIALS_PATH=/path/to/oauth-credentials.json
GENAI_API_KEY=your_genai_api_key_here
```

### For Production (Advanced):
```bash
# Add to .env file
GOOGLE_SERVICE_ACCOUNT_PATH=/path/to/service-account.json
GENAI_API_KEY=your_genai_api_key_here
```

## 🤖 GenAI Integration

The system now processes audio files with Google GenAI (Gemini) to generate transcripts. The transcript will be included in the Google Doc with custom processing that:

- Transcribes the audio
- Labels speakers as "Speaker 1", "Speaker 2" 
- Adds "Gleany Glean" marker when money-related topics are discussed
- Custom additions labeled as "Speaker Glean"

**Required:** Add `GENAI_API_KEY` to your environment variables with your Google GenAI API key.

## Without Credentials
If no credentials are configured, the feature will be skipped gracefully and the upload will still work normally.

## Troubleshooting

### Error: "The caller does not have permission" (403 Forbidden)
This means your service account lacks the required permissions:

**Solution 1: Enable APIs**
1. Go to Google Cloud Console → "APIs & Services" → "Library"
2. Search and enable:
   - **Google Docs API** ✅
   - **Google Drive API** ✅

**Solution 2: Update Service Account Role**
1. Go to "IAM & Admin" → "IAM" 
2. Find your service account email
3. Click "Edit" → Add role → **"Editor"**
4. Save changes

**Solution 3: Verify Project**
- Ensure you're using the correct Google Cloud project
- Check that the service account JSON file is from the same project where APIs are enabled

**Solution 4: Wait for Propagation**
- Permission changes can take 5-10 minutes to propagate
- Try again after a few minutes

### Testing
After making changes, restart your backend server and test the upload again. You should see:
```
INFO - Google Doc created for audio file: https://docs.google.com/document/d/...
```
