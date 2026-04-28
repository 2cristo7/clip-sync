# ClipSync — Dev Commands

## macOS

### Install app (build Release + deploy to /Applications)
```bash
cd mac
pkill -x ClipSync 2>/dev/null
xcodebuild -scheme ClipSync -configuration Release -destination 'platform=macOS' -derivedDataPath /tmp/clipsync-build build 2>&1 | grep -E "error:|BUILD (SUCCEEDED|FAILED)"
cp -R /tmp/clipsync-build/Build/Products/Release/ClipSync.app /Applications/ClipSync.app
open /Applications/ClipSync.app
```

### Check for compile errors only (fast)
```bash
cd mac
xcodebuild -scheme ClipSync -destination 'platform=macOS' build 2>&1 | grep -E "error:|BUILD (SUCCEEDED|FAILED)"
```

---

## Android

### Install debug APK on connected device
```bash
cd android
./gradlew assembleDebug && adb install -r app/build/outputs/apk/debug/app-debug.apk
```

### Check for compile errors only (no install)
```bash
cd android
./gradlew compileDebugKotlin 2>&1 | grep -E "error:|warning:"
```
