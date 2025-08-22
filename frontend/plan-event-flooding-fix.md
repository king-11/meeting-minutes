# Plan: Fix Event Flooding in Floating Window

## Problem
The app gets stuck due to excessive audio level events being emitted from the backend (potentially hundreds per second), causing:
- Too many React re-renders
- Console log flooding
- UI performance degradation

## Root Cause
- Backend emits audio-levels event for every audio chunk processed
- **DOUBLE EMISSION**: Backend emits events twice - once globally and once specifically to floating window (audio_monitor.rs lines 69 & 77)
- Main processing loop calls process_audio_with_levels in tight while loop for EACH chunk without throttling (lib.rs line 85)
- Each event triggers immediate React state update
- Console.log fires for every event
- No throttling or debouncing in place
- CSS transitions (50ms) would conflict with requestAnimationFrame (16.67ms)

## Solution: Frontend-Only Optimization

### Strategy
Keep backend unchanged but optimize frontend event handling using:
1. **requestAnimationFrame** for smooth 60fps visual updates
2. **Refs** to store audio data without triggering re-renders
3. **Debounced logging** to reduce console spam

**STATUS: NOT IMPLEMENTED** - The current code at line 44-50 in `src/app/floating/page.tsx` still directly calls `setAudioLevels` for every event without any throttling or debouncing

### Implementation Details

#### 1. Add Refs for High-Frequency Data (TypeScript)
```typescript
const audioLevelsRef = useRef<AudioLevels>({ rms: 0, peak: 0 });
const animationFrameRef = useRef<number | undefined>(undefined);
const lastLogTimeRef = useRef<number>(0);
// Performance monitoring (optional, for debugging)
const eventCountRef = useRef<number>(0);
const renderCountRef = useRef<number>(0);
```

#### 2. Update Event Listener
```typescript
const unlistenAudioLevels = listen<AudioLevels>(
  "audio-levels",
  (event) => {
    // Debounced logging - only log every 500ms
    const now = Date.now();
    if (now - lastLogTimeRef.current > 500) {
      console.log("[Floating] Audio levels - RMS:", event.payload.rms.toFixed(3));
      lastLogTimeRef.current = now;
    }
    
    // Store in ref (no re-render)
    audioLevelsRef.current = event.payload;
    
    // Schedule UI update via requestAnimationFrame
    if (!animationFrameRef.current) {
      animationFrameRef.current = requestAnimationFrame(() => {
        setAudioLevels(audioLevelsRef.current);
        animationFrameRef.current = undefined;
      });
    }
  },
);
```

#### 3. Cleanup
```typescript
// In useEffect cleanup
if (animationFrameRef.current) {
  cancelAnimationFrame(animationFrameRef.current);
  animationFrameRef.current = undefined;
}
```

#### 4. CSS Transition Adjustment
```typescript
// Remove or reduce transition duration in style
transition: "height 0.016s ease-out", // Match RAF rate or remove entirely
```

### Benefits
- **Performance**: React only re-renders at max 60fps instead of hundreds of times per second
- **Smooth UI**: Visual updates remain fluid using browser's optimal refresh rate
- **Reduced Logging**: Console only logs every 500ms instead of every event
- **No Backend Changes**: Solution is contained to frontend only

### Files to Modify
- `src/app/floating/page.tsx` - Implement requestAnimationFrame and debouncing
- `src-tauri/src/audio_monitor.rs` - Remove duplicate emission to floating window (lines 76-84)

### Optional Backend Optimization
Instead of frontend-only fixes, consider backend throttling:
- Add a timer in `audio_monitor.rs` to emit at max 30-60 Hz
- Or emit only when audio levels change significantly (threshold-based)

### Testing
1. Run the app with `RUST_LOG=debug pnpm tauri dev`
2. Start recording to trigger audio level events
3. Verify smooth audio visualization without UI freezing
4. Check console for reduced log frequency

## Implementation Status
**NOT COMPLETE** - The optimizations described in this plan have not been implemented. 

### Current Issues:
1. **Frontend (src/app/floating/page.tsx)**:
   - Logs every single audio-levels event (line 47)
   - Updates React state immediately on every event (line 48)
   - Has no requestAnimationFrame throttling
   - Has no debounced logging
   - CSS transition set to 50ms conflicts with 16.67ms RAF rate

2. **Backend (src-tauri/src/audio_monitor.rs)**:
   - Double emission: global broadcast (line 69) + floating window specific (line 77)
   - No rate limiting on emissions
   - Emits for every audio chunk in tight loop

The event flooding issue remains unresolved and is actually worse than initially understood due to double emissions.