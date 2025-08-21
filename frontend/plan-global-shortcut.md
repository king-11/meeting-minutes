# Global Shortcut with System Tray and Floating Window Implementation Plan

## ✅ IMPLEMENTATION STATUS: Phase 1 & 2 Complete (2025-08-21)

## Issue
The current application:
1. Doesn't run in the background or show in the macOS menu bar
2. Requires the main window to be open for recording
3. Lacks visual feedback when recording via keyboard shortcuts
4. Doesn't provide a SuperWhisper-like floating indicator for recording status

## Goal
Transform the application into a menu bar app with:
1. **System Tray Icon**: Always visible in macOS menu bar for quick access ✅
2. **Global Shortcut**: Cmd+Option to toggle recording from anywhere 🔄
3. **Floating Recording Window**: Small, always-on-top window showing: ✅
   - Live audio level visualization
   - Recording status
   - Duration counter
   - Confirmation when recording is saved
4. **Background Operation**: App continues running when main window is closed ✅

## The Idea
Create a complete background recording experience similar to SuperWhisper:
1. App launches to system tray (menu bar icon)
2. Main window can be opened/closed without affecting recording
3. Global shortcut triggers floating indicator window
4. Visual feedback through audio level meters during recording
5. Success confirmation when recording is saved

## Business Requirements

### 1. System Tray (Menu Bar)
- **Icon**: Custom icon in macOS menu bar
- **Menu Options**:
  - Start/Stop Recording
  - Open Main Window
  - Settings
  - Quit
- **Visual States**: Different icons for idle/recording states

### 2. Global Shortcut
- **Keys**: Cmd+Option (customizable in future)
- **Behavior**: Toggle recording + show floating window
- **Feedback**: Immediate visual response

### 3. Floating Recording Window
- **Size**: Compact (200x80px approximate)
- **Position**: Top-right corner by default, draggable
- **Contents**:
  - Audio level bars (animated)
  - Recording time counter
  - Stop button (optional)
  - "Saved locally" confirmation message
- **Behavior**:
  - Appears when recording starts
  - Stays on top of all windows
  - Auto-hides 2 seconds after recording stops (after showing confirmation)

### 4. Background Operation
- App doesn't quit when main window closes
- Recording continues even if windows are closed
- Only quits via menu bar "Quit" option

## Architecture Changes

### New Components Needed

1. **System Tray Manager** (Rust)
   - Initialize tray icon
   - Handle menu interactions
   - Update icon based on state

2. **Floating Window** (New Tauri Window)
   - Separate window configuration
   - Custom React component for UI
   - Real-time audio level updates

3. **Audio Level Monitor** (Rust)
   - Calculate RMS/peak levels from audio buffer
   - Emit events to floating window
   - 60fps update rate for smooth visualization

4. **Window Manager** (Rust)
   - Control floating window visibility
   - Position management
   - Focus handling

## Implementation Details

### 1. Dependencies to Add

#### ⚠️ CRITICAL UPDATES FOR TAURI V2:

#### Rust Dependencies (src-tauri/Cargo.toml)
```toml
[dependencies]
# IMPORTANT: Must include "tray-icon" feature for tray functionality!
tauri = { version = "2.6.2", features = ["macos-private-api", "protocol-asset", "tray-icon"] }
tauri-plugin-global-shortcut = "2.3.0"  # Updated version
tauri-plugin-store = "2.3.0"  # Updated version for window position persistence

# Also update macOS-specific dependencies
[target.'cfg(target_os = "macos")'.dependencies]
tauri = { version = "2.6.2", features = ["protocol-asset", "macos-private-api", "tray-icon"] }

[dev-dependencies]
mockall = "0.11.4"  # For mocking in tests
tauri = { version = "2.6.2", features = ["test"] }
```

#### JavaScript Dependencies (package.json)
```json
"@tauri-apps/plugin-global-shortcut": "^2.3.0",
"@testing-library/react": "^14.0.0",
"@testing-library/jest-dom": "^6.0.0",
"vitest": "^1.0.0"
```

## 🚨 CRITICAL TAURI V2 GOTCHAS (NOT IN ORIGINAL PLAN)

### API Changes from Tauri v1 to v2:
1. **Tray API**: 
   - `SystemTray` → `TrayIconBuilder`
   - `app.tray()` → `app.tray_by_id("main")`
   - `CustomMenuItem::new().text()` → `MenuItemBuilder::with_id()`

2. **Window API**:
   - `app.get_window()` → `app.get_webview_window()`
   - `app.app_handle()` → `app.handle()`

3. **Event API**:
   - `app.emit_to()` → `app.emit()` (requires `Emitter` trait import)

4. **Store API**:
   - Requires `defaults` parameter: `{ defaults: {}, autoSave: false }`

### Platform-Specific Configuration:
1. **LSUIElement (Menu Bar App)**:
   - ❌ NOT in `tauri.conf.json` bundle.macOS section
   - ✅ Must be in `Info.plist` file:
   ```xml
   <key>LSUIElement</key>
   <true/>
   ```

### Required Feature Flags:
```toml
# MUST include "tray-icon" feature or tray won't compile!
tauri = { version = "2.6.2", features = ["tray-icon"] }
```

### TypeScript/Next.js Gotchas:
1. **CSS Properties**: Use spread with type assertion for webkit properties
   ```typescript
   style={{ ...{ WebkitAppRegion: 'drag' } as any }}
   ```

2. **Window Position**: Use `LogicalPosition` class, not plain object
   ```typescript
   import { LogicalPosition } from '@tauri-apps/api/window';
   new LogicalPosition(x, y)
   ```

3. **SSR Issues**: Guard Tauri APIs with `typeof window !== 'undefined'`

### Build Requirements:
- **Full Xcode Required**: Command Line Tools not sufficient for audio libraries
- Install with: `xcode-select --install` then install Xcode from App Store

### 2. Files to Create/Modify

#### A. New Files to Create

**src-tauri/src/tray.rs**
```rust
// System tray management module
pub fn create_tray(app: &AppHandle) -> Result<()>
pub fn update_tray_icon(recording: bool)
pub fn create_tray_menu() -> Menu
```

**src-tauri/src/audio_monitor.rs**
```rust
// Audio level monitoring
pub fn calculate_audio_levels(buffer: &[f32]) -> AudioLevels
pub fn start_level_monitoring()
```

**src/app/floating/page.tsx**
```typescript
// Floating window UI component
// Audio level visualization
// Recording timer
// Save confirmation
// Window position persistence:
//   - Load saved position on mount
//   - Save position on drag end
//   - Store in Tauri store: { x: number, y: number }
```

**src/components/AudioLevelMeter.tsx**
```typescript
// Reusable audio level visualization component
```

#### B. Files to Modify

**src-tauri/tauri.conf.json**
```json
{
  "app": {
    "windows": [
      {
        "title": "meetily",
        "label": "main",
        // ... existing config
      },
      {
        "title": "",
        "label": "floating",
        "url": "/floating",
        "width": 220,
        "height": 90,
        "resizable": false,
        "alwaysOnTop": true,
        "decorations": false,
        "transparent": true,
        "skipTaskbar": true,
        "visible": false,
        "center": false,
        "x": 100,
        "y": 100
      }
    ],
    "trayIcon": {
      "iconPath": "icons/tray-icon.png",
      "iconAsTemplate": true,
      "menuOnLeftClick": false
    },
    "macOSPrivateApi": true,
    "security": {
      "capabilities": [{
        "permissions": [
          // ... existing permissions
          "global-shortcut:allow-register",
          "global-shortcut:allow-unregister",
          "global-shortcut:allow-is-registered",
          "core:tray:allow-new",
          "core:tray:allow-set-icon",
          "core:tray:allow-set-menu",
          "core:tray:allow-set-tooltip",
          "core:window:allow-show",
          "core:window:allow-hide",
          "core:window:allow-close",
          "core:window:allow-set-position",
          "core:window:allow-set-always-on-top"
        ]
      }]
    }
  },
  "bundle": {
    "macOS": {
      "minimumSystemVersion": "10.15",
      "exceptionDomain": ""
      // ⚠️ GOTCHA: LSUIElement NOT supported here in Tauri v2!
      // Must be added to Info.plist instead
    }
  }
}
```

**src-tauri/src/lib.rs**
- Add tray module import
- Add audio monitor module
- Initialize system tray in setup
- Register global shortcut
- Add commands:
  - `toggle_recording_with_ui_feedback`
  - `get_audio_levels`
  - `show_floating_window`
  - `hide_floating_window`
  - `update_floating_position`
  - `save_window_position` - Store position in Tauri store
  - `get_window_position` - Retrieve saved position

**src-tauri/src/audio/core.rs**
- Add audio level calculation to existing audio capture
- Emit 'audio-levels' events at 60fps
- Store peak and RMS values

**src/app/page.tsx**
- Handle window close to hide instead of quit
- Listen for tray menu events
- Sync state with floating window

**next.config.mjs**
- Add /floating route configuration

### 3. Test Implementation

#### Test File: src-tauri/src/tests/system_integration_tests.rs
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tray_initialization() {
        // Test that system tray initializes correctly
        // Verify menu items are created
    }

    #[test]
    fn test_floating_window_lifecycle() {
        // Test floating window show/hide
        // Verify position persistence
        // Test always-on-top behavior
    }

    #[test]
    fn test_window_position_persistence() {
        // Test saving window position to store
        let result = save_window_position(app, 100, 200);
        assert!(result.is_ok());
        
        // Test retrieving saved position
        let (x, y) = get_window_position(app).unwrap();
        assert_eq!(x, 100);
        assert_eq!(y, 200);
    }

    #[test]
    fn test_default_window_position() {
        // Test default position when no saved position exists
        let (x, y) = get_window_position(app).unwrap();
        
        // Should be top-right corner
        let display = app.primary_monitor().unwrap();
        assert_eq!(x, display.size.width - 240);
        assert_eq!(y, 20);
    }

    #[test]
    fn test_audio_level_calculation() {
        // Test RMS calculation accuracy
        // Test peak detection
        // Verify 60fps emission rate
    }

    #[test]
    fn test_background_recording() {
        // Test recording continues when main window closed
        // Verify tray icon updates
        // Test state persistence
    }

    #[test]
    fn test_shortcut_with_floating_window() {
        // Test shortcut shows floating window
        // Test UI updates in floating window
        // Test auto-hide after recording
    }
}
```

#### Frontend Test File: src/__tests__/floatingWindow.test.tsx
```typescript
describe('Floating Window', () => {
  it('should display audio levels in real-time', async () => {
    // Test audio level meter updates
  });

  it('should show recording duration', async () => {
    // Test timer functionality
  });

  it('should display save confirmation', async () => {
    // Test confirmation message appears
    // Test auto-hide after 2 seconds
  });

  it('should be draggable', async () => {
    // Test window can be repositioned
    // Test position persistence
  });

  it('should persist window position on drag', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    const mockInvoke = vi.mocked(invoke);
    
    // Simulate dragging window to new position
    await simulateWindowDrag(150, 250);
    
    // Verify save_window_position was called
    expect(mockInvoke).toHaveBeenCalledWith('save_window_position', {
      x: 150,
      y: 250
    });
  });

  it('should restore saved window position on mount', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    const mockInvoke = vi.mocked(invoke);
    
    // Mock saved position
    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'get_window_position') {
        return Promise.resolve({ x: 300, y: 150 });
      }
    });
    
    render(<FloatingWindow />);
    
    // Verify position was requested
    expect(mockInvoke).toHaveBeenCalledWith('get_window_position');
    
    // Verify window is positioned correctly
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('set_window_position', {
        x: 300,
        y: 150
      });
    });
  });
});
```

### 4. Implementation Steps (TDD Approach)

#### Phase 1: System Tray Setup
1. Create tray icon assets:
   - `icons/tray-icon.png` - Default idle state icon (16x16 or 22x22 for macOS)
   - `icons/tray-recording.png` - Recording state icon (red dot or similar)
   - `icons/tray-idle.png` - Alternative idle state (if needed)
   - Use iconAsTemplate: true for macOS native appearance
2. Implement basic tray menu
3. Configure LSUIElement for background operation
4. Test app stays running when window closes

#### Phase 2: Floating Window Infrastructure
1. Create floating window configuration
2. Build basic floating window UI component
3. Implement window show/hide commands
4. Add position persistence:
   - Create Tauri store for window preferences
   - Save position on drag end
   - Restore position on window show
   - Default to top-right corner if no saved position
5. Test window lifecycle

#### Phase 3: Audio Level Monitoring
1. Implement RMS/peak calculation in Rust
2. Add event emission for levels
3. Create AudioLevelMeter component
4. Test real-time updates

#### Phase 4: Global Shortcut Integration
1. Register Cmd+Option shortcut
2. Connect to toggle recording
3. Trigger floating window display
4. Test shortcut from background

#### Phase 5: Polish & Confirmation
1. Add recording timer to floating window
2. Implement "Saved locally" confirmation
3. Add smooth animations
4. Test complete user flow

### 5. Verification Strategy

#### Automated Test Suite

##### A. Unit Tests (Rust) - src-tauri/src/tests/

**test_tray.rs**
```rust
#[cfg(test)]
mod tray_tests {
    use super::*;
    use mockall::*;
    
    #[test]
    fn test_tray_menu_creation() {
        // Given: A mock app handle
        let mock_app = create_mock_app_handle();
        
        // When: Creating tray menu
        let menu = create_tray_menu();
        
        // Then: Menu should have expected items
        assert!(menu.has_item("start_recording"));
        assert!(menu.has_item("open_main_window"));
        assert!(menu.has_item("settings"));
        assert!(menu.has_item("quit"));
    }
    
    #[test]
    fn test_tray_icon_updates_on_recording_state() {
        // Given: Tray is initialized
        let tray = create_test_tray();
        
        // When: Recording starts
        update_tray_icon(true);
        
        // Then: Icon should be recording icon
        assert_eq!(tray.get_icon_path(), "icons/tray-recording.png");
        
        // When: Recording stops
        update_tray_icon(false);
        
        // Then: Icon should be idle icon
        assert_eq!(tray.get_icon_path(), "icons/tray-idle.png");
    }
}
```

**test_audio_monitor.rs**
```rust
#[cfg(test)]
mod audio_monitor_tests {
    use super::*;
    
    #[test]
    fn test_rms_calculation() {
        // Given: Known audio samples
        let samples = vec![0.5, -0.5, 0.3, -0.3, 0.1, -0.1];
        
        // When: Calculating RMS
        let rms = calculate_rms(&samples);
        
        // Then: RMS should be correct
        assert!((rms - 0.3464).abs() < 0.001);
    }
    
    #[test]
    fn test_peak_detection() {
        // Given: Audio samples with known peak
        let samples = vec![0.1, 0.5, 0.9, 0.3, -0.7, 0.2];
        
        // When: Finding peak
        let peak = find_peak(&samples);
        
        // Then: Peak should be 0.9
        assert_eq!(peak, 0.9);
    }
    
    #[test]
    fn test_audio_level_event_emission() {
        // Given: Mock event emitter
        let mut mock_emitter = MockEventEmitter::new();
        mock_emitter.expect_emit()
            .with(eq("audio-levels"), any())
            .times(1)
            .return_const(Ok(()));
        
        // When: Processing audio buffer
        process_audio_with_levels(&samples, &mock_emitter);
        
        // Then: Event should be emitted (verified by mock)
    }
}
```

**test_global_shortcut.rs**
```rust
#[cfg(test)]
mod shortcut_tests {
    use super::*;
    use tauri::test::*;
    
    #[test]
    fn test_shortcut_registration() {
        // Given: Mock shortcut manager
        let mut mock_manager = MockShortcutManager::new();
        mock_manager.expect_register()
            .with(eq("Cmd+Option"))
            .times(1)
            .return_const(Ok(()));
        
        // When: Registering shortcut
        let result = register_recording_shortcut(&mock_manager);
        
        // Then: Should succeed
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_toggle_recording_via_shortcut() {
        // Given: Recording is not active
        RECORDING_FLAG.store(false, Ordering::SeqCst);
        let mock_window_manager = MockWindowManager::new();
        mock_window_manager.expect_show_window()
            .with(eq("floating"))
            .times(1)
            .return_const(Ok(()));
        
        // When: Shortcut is triggered
        handle_shortcut_triggered(&mock_window_manager);
        
        // Then: Recording should start and floating window should show
        assert!(RECORDING_FLAG.load(Ordering::SeqCst));
    }
}
```

##### B. Integration Tests (Rust) - src-tauri/tests/

**integration_test.rs**
```rust
#[test]
fn test_app_stays_running_on_window_close() {
    // Given: App is running with system tray
    let app = create_test_app();
    app.setup_tray();
    
    // When: Main window is closed
    app.get_window("main").unwrap().close();
    
    // Then: App should still be running
    assert!(app.is_running());
    assert!(app.tray().is_some());
}

#[test]
fn test_floating_window_lifecycle() {
    // Given: App with floating window configured
    let app = create_test_app();
    let floating = app.get_window("floating").unwrap();
    
    // When: Starting recording
    app.trigger_recording_start();
    
    // Then: Floating window should be visible
    assert!(floating.is_visible());
    assert!(floating.is_always_on_top());
    
    // When: Stopping recording
    app.trigger_recording_stop();
    thread::sleep(Duration::from_secs(3));
    
    // Then: Floating window should be hidden
    assert!(!floating.is_visible());
}

#[test]
fn test_window_position_persistence_integration() {
    // Given: App with floating window
    let app = create_test_app();
    let floating = app.get_window("floating").unwrap();
    
    // When: Window is moved to new position
    floating.set_position(LogicalPosition::new(200, 300)).unwrap();
    
    // Then: Position should be saved to store
    let store = StoreBuilder::new("window_preferences.json").build(&app);
    let saved_pos = store.get("floating_window_position").unwrap();
    assert_eq!(saved_pos["x"], 200);
    assert_eq!(saved_pos["y"], 300);
    
    // When: Window is hidden and shown again
    floating.hide().unwrap();
    floating.show().unwrap();
    
    // Then: Position should be restored
    let position = floating.outer_position().unwrap();
    assert_eq!(position.x, 200);
    assert_eq!(position.y, 300);
}
```

##### C. Frontend Tests (TypeScript) - src/__tests__/

**FloatingWindow.test.tsx**
```typescript
import { render, screen, waitFor } from '@testing-library/react';
import { vi } from 'vitest';
import FloatingWindow from '@/app/floating/page';
import { mockTauriAPI } from './mocks/tauriMocks';

describe('Floating Window Component', () => {
  beforeEach(() => {
    mockTauriAPI();
  });

  it('should display audio levels when receiving events', async () => {
    const { emit } = await import('@tauri-apps/api/event');
    const mockEmit = vi.mocked(emit);
    
    render(<FloatingWindow />);
    
    // Emit audio level event
    await mockEmit('audio-levels', {
      payload: { rms: 0.5, peak: 0.8 }
    });
    
    await waitFor(() => {
      const levelMeter = screen.getByTestId('audio-level-meter');
      expect(levelMeter).toHaveStyle({ height: '80%' });
    });
  });

  it('should show recording timer', async () => {
    render(<FloatingWindow />);
    
    // Start recording
    await mockTauriAPI.invoke('start_recording');
    
    // Wait for timer to update
    await waitFor(() => {
      expect(screen.getByText(/0:01/)).toBeInTheDocument();
    }, { timeout: 1500 });
  });

  it('should display save confirmation and auto-hide', async () => {
    vi.useFakeTimers();
    const { invoke } = await import('@tauri-apps/api/core');
    const mockInvoke = vi.mocked(invoke);
    
    render(<FloatingWindow />);
    
    // Stop recording
    await mockInvoke('stop_recording');
    
    // Check confirmation appears
    await waitFor(() => {
      expect(screen.getByText('Saved locally')).toBeInTheDocument();
    });
    
    // Fast-forward 2 seconds
    vi.advanceTimersByTime(2000);
    
    // Verify hide_window was called
    expect(mockInvoke).toHaveBeenCalledWith('hide_floating_window');
    
    vi.useRealTimers();
  });
});
```

**SystemTray.test.tsx**
```typescript
import { vi } from 'vitest';
import { mockTauriAPI } from './mocks/tauriMocks';

describe('System Tray Integration', () => {
  beforeEach(() => {
    mockTauriAPI();
  });

  it('should handle tray menu clicks', async () => {
    const { emit, listen } = await import('@tauri-apps/api/event');
    const mockListen = vi.mocked(listen);
    const mockInvoke = vi.mocked(invoke);
    
    // Setup listener
    const callback = vi.fn();
    await mockListen('tray-menu-click', callback);
    
    // Simulate menu click
    await emit('tray-menu-click', { 
      payload: { item_id: 'start_recording' }
    });
    
    // Verify recording started
    expect(mockInvoke).toHaveBeenCalledWith('start_recording');
  });
});
```

**mocks/tauriMocks.ts**
```typescript
export function mockTauriAPI() {
  const mockInvoke = vi.fn((cmd: string, args?: any) => {
    switch(cmd) {
      case 'is_recording':
        return Promise.resolve(false);
      case 'start_recording':
        return Promise.resolve();
      case 'stop_recording':
        return Promise.resolve();
      case 'show_floating_window':
        return Promise.resolve();
      case 'hide_floating_window':
        return Promise.resolve();
      case 'get_audio_levels':
        return Promise.resolve({ rms: 0.3, peak: 0.6 });
      default:
        return Promise.resolve();
    }
  });

  const mockListen = vi.fn((event: string, handler: Function) => {
    // Store handlers for event simulation
    return Promise.resolve(() => {});
  });

  const mockEmit = vi.fn();

  vi.mock('@tauri-apps/api/core', () => ({
    invoke: mockInvoke
  }));

  vi.mock('@tauri-apps/api/event', () => ({
    listen: mockListen,
    emit: mockEmit
  }));

  return { mockInvoke, mockListen, mockEmit };
}
```

#### Test Execution Commands
```bash
# Run all Rust tests with coverage
cd src-tauri
cargo test --all-features -- --test-threads=1 --nocapture
cargo tarpaulin --out Html

# Run frontend tests with coverage
pnpm test --coverage
pnpm test:e2e  # For e2e tests with actual Tauri app

# Run specific test suites
cargo test tray_tests
cargo test audio_monitor_tests
cargo test shortcut_tests
pnpm test FloatingWindow
pnpm test SystemTray

# Continuous test watch
cargo watch -x test
pnpm test --watch
```

#### Test Coverage Requirements
- Unit Tests: >80% coverage
- Integration Tests: Critical paths covered
- E2E Tests: Main user flows covered

#### CI/CD Verification
```yaml
# .github/workflows/test.yml
name: Test Suite
on: [push, pull_request]
jobs:
  rust-tests:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v2
      - name: Run Rust tests
        run: |
          cd src-tauri
          cargo test --all-features
          
  frontend-tests:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v2
      - name: Run Frontend tests
        run: |
          pnpm install
          pnpm test --coverage
          
  integration-tests:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v2
      - name: Run Integration tests
        run: |
          pnpm build
          pnpm test:e2e
```

### 6. Performance Considerations

1. **Audio Level Updates**: Throttle to 60fps max to prevent CPU overuse
2. **Floating Window**: Use CSS transforms for animations (GPU accelerated)
3. **Memory**: Clear audio buffers after level calculation
4. **Event Emissions**: Batch updates when possible

### 7. Window Position Persistence Implementation

**Store Structure (Tauri Store)**
```json
{
  "floating_window_position": {
    "x": 100,
    "y": 100
  }
}
```

**Implementation in src-tauri/src/window_manager.rs**
```rust
use tauri_plugin_store::StoreBuilder;

pub async fn save_window_position(app: AppHandle, x: i32, y: i32) -> Result<()> {
    let store = StoreBuilder::new("window_preferences.json").build(app);
    store.set("floating_window_position", json!({ "x": x, "y": y }));
    store.save()?;
    Ok(())
}

pub async fn get_window_position(app: AppHandle) -> Result<(i32, i32)> {
    let store = StoreBuilder::new("window_preferences.json").build(app);
    let position = store.get("floating_window_position")
        .unwrap_or(json!({ "x": -1, "y": -1 }));
    
    // Return saved position or calculate default (top-right corner)
    if position["x"] == -1 {
        let display = app.primary_monitor()?;
        let x = display.size.width - 240; // 220px window + 20px margin
        let y = 20; // 20px from top
        Ok((x, y))
    } else {
        Ok((position["x"].as_i64()? as i32, position["y"].as_i64()? as i32))
    }
}
```

### 8. Future Enhancements (Not in MVP)
- Customizable shortcut keys
- Multiple floating window themes
- Floating window opacity settings
- Audio level sensitivity adjustment
- Click-through floating window option
- Multiple monitor support for floating window position

### 9. Success Criteria
- [x] App runs as menu bar application ✅
- [ ] Global shortcut works from any application 🔄
- [x] Floating window provides clear recording feedback ✅
- [ ] Audio levels display in real-time 🔄
- [x] Save confirmation is shown ✅
- [x] All existing functionality remains intact ✅
- [ ] Performance remains smooth (60fps animations)
- [ ] No memory leaks during extended use
- [ ] All tests pass with >80% coverage

## 📝 IMPLEMENTATION COMPLETED (Phase 1 & 2)

### What Was Successfully Implemented:

#### Phase 1: System Tray ✅
- Created tray icon assets (tray-icon.png, tray-idle.png, tray-recording.png)
- Implemented `src-tauri/src/tray.rs` with full menu functionality
- Configured LSUIElement in Info.plist for menu bar behavior
- Integrated tray initialization in main app setup
- Menu options: Start/Stop Recording, Open Main Window, Settings, Quit

#### Phase 2: Floating Window ✅
- Created floating window configuration in tauri.conf.json
- Built React component at `src/app/floating/page.tsx`
- Implemented window manager at `src-tauri/src/window_manager.rs`
- Added position persistence with Tauri store
- Window features: 220x90px, transparent, always-on-top, draggable

### Key Files Created/Modified:
```
✅ src-tauri/src/tray.rs (NEW)
✅ src-tauri/src/window_manager.rs (NEW)
✅ src/app/floating/page.tsx (NEW)
✅ src-tauri/Info.plist (MODIFIED - Added LSUIElement)
✅ src-tauri/tauri.conf.json (MODIFIED - Added floating window)
✅ src-tauri/Cargo.toml (MODIFIED - Added tray-icon feature)
✅ src-tauri/src/lib.rs (MODIFIED - Integrated modules)
```

### Confirmed Working:
- Application runs as menu bar app (no dock icon)
- System tray appears in menu bar with functional menu
- Floating window configuration ready
- All TypeScript and Rust compilation issues resolved
- App successfully builds and runs with `pnpm tauri dev`

### Remaining Phases:
- Phase 3: Audio Level Monitoring (pending)
- Phase 4: Global Shortcut Integration (pending)

### Run Commands:
```bash
# Development
./clean_run.sh debug

# Build
./clean_build.sh

# Direct Tauri commands
pnpm tauri dev
pnpm tauri build
```