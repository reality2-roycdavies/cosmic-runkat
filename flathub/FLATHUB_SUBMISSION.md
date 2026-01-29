# Flathub Submission Guide for cosmic-runkat v1.0.0

## Pre-Submission Checklist

### Repository Requirements

- ✅ App ID matches reverse DNS: `io.github.reality2_roycdavies.cosmic-runkat`
- ✅ Manifest file named: `io.github.reality2_roycdavies.cosmic-runkat.yml`
- ✅ Uses git tag source (v1.0.0)
- ✅ Includes cargo-sources.json for offline builds
- ✅ freedesktop runtime 25.08
- ✅ rust-stable SDK extension

### Metadata Requirements

- ✅ AppStream metainfo.xml present
- ✅ Desktop file present
- ✅ Screenshots available (need to verify URLs work)
- ✅ Icons (SVG + symbolic) present
- ✅ License specified (MIT)
- ✅ Developer information included
- ✅ Release notes for v1.0.0 added

### Permissions (Minimal Principle)

- ✅ `--share=ipc` - Required for Wayland
- ✅ `--socket=wayland` - Display access
- ✅ `--socket=fallback-x11` - X11 compatibility
- ✅ `--talk-name=org.kde.StatusNotifierWatcher` - System tray
- ✅ `--filesystem=host:ro` - Read /proc/stat for CPU
- ✅ `--filesystem=~/.config/cosmic:ro` - Read COSMIC theme
- ✅ `--filesystem=~/.config/cosmic-runkat:create` - App config
- ✅ `--filesystem=~/.config/autostart:create` - Autostart entry

**Note:** No broad permissions like `--filesystem=home` or `--socket=session-bus`

### Build Verification

Test the manifest builds correctly:

```bash
cd flathub
flatpak-builder --force-clean build-dir io.github.reality2_roycdavies.cosmic-runkat.yml
flatpak-builder --run build-dir io.github.reality2_roycdavies.cosmic-runkat.yml cosmic-runkat --version
```

---

## Flathub Submission Process

### Step 1: Fork flathub/flathub Repository

```bash
# On GitHub
1. Go to: https://github.com/flathub/flathub
2. Click "Fork"
3. Clone your fork:
   git clone https://github.com/YOUR-USERNAME/flathub.git
   cd flathub
```

### Step 2: Create Application Branch

```bash
git checkout -b io.github.reality2_roycdavies.cosmic-runkat
```

### Step 3: Add Application Files

```bash
mkdir io.github.reality2_roycdavies.cosmic-runkat
cd io.github.reality2_roycdavies.cosmic-runkat

# Copy manifest
cp /path/to/cosmic-runkat/flathub/io.github.reality2_roycdavies.cosmic-runkat.yml .

# Copy cargo sources
cp /path/to/cosmic-runkat/flathub/cargo-sources.json .

# Create flathub.json (required)
cat > flathub.json << 'EOF'
{
  "only-arches": ["x86_64", "aarch64"]
}
EOF
```

### Step 4: Verify Build

```bash
# From flathub repo root
flatpak-builder --force-clean build-dir \\
  io.github.reality2_roycdavies.cosmic-runkat/io.github.reality2_roycdavies.cosmic-runkat.yml

# Test run
flatpak-builder --run build-dir \\
  io.github.reality2_roycdavies.cosmic-runkat/io.github.reality2_roycdavies.cosmic-runkat.yml \\
  cosmic-runkat --version
```

### Step 5: Commit and Push

```bash
git add io.github.reality2_roycdavies.cosmic-runkat/
git commit -m "Add io.github.reality2_roycdavies.cosmic-runkat

RunKat - A cute running cat CPU indicator for COSMIC desktop

Initial Flathub submission of v1.0.0

App features:
- Animated cat tray icon that runs faster with CPU usage
- Sleep mode when CPU is idle
- Optional CPU percentage display
- Theme-aware (automatic dark/light adaptation)
- Panel size aware
- Configurable thresholds

Technical highlights:
- Async event-driven architecture (tokio)
- 0.1-0.2% CPU usage
- 20 comprehensive tests
- Production-ready error handling
- Structured logging

COSMIC desktop only - requires COSMIC panel for system tray.

See: https://github.com/reality2-roycdavies/cosmic-runkat"

git push origin io.github.reality2_roycdavies.cosmic-runkat
```

### Step 6: Create Pull Request

1. Go to: https://github.com/flathub/flathub
2. Click "New Pull Request"
3. Select your fork and branch
4. Title: "Add io.github.reality2_roycdavies.cosmic-runkat"
5. Description:

```markdown
# cosmic-runkat - Running Cat CPU Indicator for COSMIC

## Application Description

RunKat displays an animated cat in the system tray that runs faster when CPU usage is higher. When CPU is idle, the cat sleeps. Optional CPU percentage display.

**COSMIC Desktop Only** - Requires COSMIC panel for system tray functionality.

## First Submission

- [x] I've read the [app submission guidelines](https://docs.flathub.org/docs/for-app-authors/submission)
- [x] I've verified the manifest builds successfully
- [x] I've tested the application
- [x] AppStream metadata is valid
- [x] Screenshots are provided
- [x] Icons are included (SVG + symbolic)
- [x] Permissions follow minimal principle

## Technical Details

- **Version:** 1.0.0
- **License:** MIT
- **Language:** Rust
- **Runtime:** freedesktop 25.08
- **Architecture:** x86_64, aarch64

## Performance

- CPU usage: 0.1-0.2% (highly optimized)
- Memory: ~11 MB
- Async event-driven architecture
- Production-ready code quality

## Links

- **Homepage:** https://github.com/reality2-roycdavies/cosmic-runkat
- **Issues:** https://github.com/reality2-roycdavies/cosmic-runkat/issues

## Educational Value

This app includes extensive documentation as an educational resource for AI-assisted development.

## Notes

COSMIC desktop requirement is clearly stated in AppStream metadata and README. Will not function on other DEs (requires COSMIC panel's StatusNotifierWatcher).
```

---

## Pre-Submission Verification

### Run These Commands

```bash
# 1. Validate AppStream metadata
flatpak run org.freedesktop.appstream-glib validate \\
  resources/io.github.reality2_roycdavies.cosmic-runkat.metainfo.xml

# 2. Validate desktop file
desktop-file-validate \\
  resources/io.github.reality2_roycdavies.cosmic-runkat.desktop

# 3. Build from git tag (simulates Flathub)
cd flathub
flatpak-builder --force-clean build-dir \\
  io.github.reality2_roycdavies.cosmic-runkat.yml

# 4. Test installation
flatpak-builder --user --install --force-clean build-dir \\
  io.github.reality2_roycdavies.cosmic-runkat.yml

# 5. Test run
flatpak run io.github.reality2_roycdavies.cosmic-runkat --version
flatpak run io.github.reality2_roycdavies.cosmic-runkat --tray
```

---

## Common Flathub Review Items

### Likely Review Comments

1. **Screenshots:**
   - Verify URLs are accessible
   - Should be HTTPS
   - Should be permanent (GitHub raw URLs are fine)

2. **Permissions Justification:**
   - `--filesystem=host:ro` - Explain: "Required to read /proc/stat for CPU monitoring"
   - `--filesystem=~/.config/cosmic:ro` - Explain: "Required to read COSMIC theme configuration"

3. **COSMIC Desktop Requirement:**
   - Clearly stated in metainfo.xml
   - Won't function without COSMIC panel
   - This is acceptable (like GNOME-only apps)

4. **Build Time:**
   - First build may be slow (libcosmic is large)
   - This is normal for COSMIC apps

### Potential Questions & Answers

**Q: Why does it need host filesystem access?**
A: Reads /proc/stat for CPU monitoring. This is read-only and standard for system monitors.

**Q: Will it work on GNOME/KDE?**
A: No, COSMIC-only. Requires COSMIC panel's StatusNotifierWatcher. This is clearly documented.

**Q: Why are screenshots from GitHub raw URLs?**
A: Permanent URLs, versioned with git. Alternative is to commit to flathub repo.

**Q: Why such large dependency set (libcosmic)?**
A: COSMIC apps require libcosmic for native look and feel. Shared across COSMIC apps.

---

## Post-Submission

### Expected Timeline

- **Initial Review:** 1-3 days
- **Build Testing:** Automatic via Flathub CI
- **Review Comments:** Respond within 48 hours
- **Approval:** Usually 1-2 weeks for new apps
- **Publication:** Automatic after approval

### After Approval

1. **Flathub automatically builds** from your GitHub tag
2. **Users can install** via: `flatpak install flathub io.github.reality2_roycdavies.cosmic-runkat`
3. **Updates** done by submitting PRs to flathub/io.github.reality2_roycdavies.cosmic-runkat

---

## Maintaining on Flathub

### For Future Releases

1. Tag new release in your repo (e.g., v1.1.0)
2. Update manifest in flathub repo:
   - Change tag: v1.0.0 → v1.1.0
   - Update commit hash
3. Update cargo-sources.json (regenerate with flatpak-cargo-generator.py)
4. Add release entry to metainfo.xml
5. Submit PR to flathub repo

**Automated tools:**
```bash
# Regenerate cargo sources
python3 flatpak-cargo-generator.py Cargo.lock -o cargo-sources.json

# Get commit hash for tag
git rev-parse v1.1.0
```

---

## Flathub Quality Standards Met

### Code Quality ✅
- Production-ready architecture
- Comprehensive testing (20 tests)
- Clean error handling
- Professional logging

### Documentation ✅
- Detailed README
- User migration guide
- AppStream metadata complete
- Developer documentation

### User Experience ✅
- Clear application description
- Screenshots provided
- Minimal permissions requested
- COSMIC requirement clearly stated

### Technical Standards ✅
- Follows Flatpak best practices
- Offline build with vendored sources
- Proper permission scoping
- Standard metadata format

---

## Additional Notes

### COSMIC Desktop Requirement

**Important:** This app is **COSMIC-only** by design. It:
- Uses COSMIC's StatusNotifierWatcher (system tray)
- Reads COSMIC theme configuration
- Uses COSMIC panel size detection
- Won't function on other DEs

**Precedent:** Similar to apps like:
- GNOME Extensions (GNOME-only)
- KDE Plasmoids (KDE-only)
- COSMIC-specific apps are acceptable on Flathub

### Performance Claims

All performance metrics in release notes are:
- ✅ Measured (not estimated)
- ✅ Documented in case study
- ✅ Reproducible via testing

### Educational Aspect

The comprehensive documentation is part of the project's value proposition:
- Real-world AI development case study
- Architecture reference
- Learning resource for developers

---

## Quick Reference

**Manifest Location:** `flathub/io.github.reality2_roycdavies.cosmic-runkat.yml`  
**Metainfo:** `resources/io.github.reality2_roycdavies.cosmic-runkat.metainfo.xml`  
**Desktop File:** `resources/io.github.reality2_roycdavies.cosmic-runkat.desktop`  
**Icons:** `resources/io.github.reality2_roycdavies.cosmic-runkat.svg` (+ symbolic)

**Git Tag:** v1.0.0  
**Commit Hash:** a0fe560e685850b930b7096bbeee858d6027f4ee

---

## Contact for Issues

**Maintainer:** Dr. Roy C. Davies  
**GitHub:** https://github.com/reality2-roycdavies/cosmic-runkat  
**Issues:** https://github.com/reality2-roycdavies/cosmic-runkat/issues

---

*Last Updated: 2026-01-29*  
*Version: 1.0.0*
