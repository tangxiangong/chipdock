# ChipDock NSH workbench

## Scope and ownership

Desktop: macOS, Windows and Linux. No mobile, web or TUI target. Keep the firmware document, target setup and flash actions together. Serial uses a connection sidebar, terminal and multiline composer; secondary workflows use dialogs, and memory, logs, settings and session plots have independent windows.

Rust owns the whole application: preferences, session drafts, connection state, jobs, protocol validation, bounded records, plot samples, signed updates and NSH views. No TSX, QuickJS UI or downloaded interface packages. Keep domain logic independent of NSH and expose it to retained components through signals.

## Foundations

This is a framework migration of the already tuned GPUI workbench, with the current macOS client as the reference. Preserve the existing page hierarchy, title bar, firmware/settings split, connection rail, terminal/composer split, menus, dialogs and independent tool windows. NSH's default type and control scales do not replace those dimensions.

Use system sans for interface text and system monospace for bytes, addresses, identifiers and technical input. The reference uses 13 pt body and regular button text, 12 pt callouts/captions, and 10 pt native secondary labels where the source calls for them. Small native controls retain their separately measured sizes (including 11 pt baud input and native table headers, and 12 pt parameter picker text). The inherited line-height ratio is 1.2; an explicitly measured native multiline label keeps its reference line height. The native monospaced workflow editor uses a measured 16.5 pt line height. Do not substitute a 12/14/16/20 type scale.

Regular push buttons are 24 pt, accent capsules are 28 pt, and small controls are 20 pt. The title bar is 52 pt. Rows keep their owning reference geometry rather than a universal row height: for example, firmware byte rows are 17 pt, the serial terminal uses 32 pt rows, and diagnostic grouped forms use the measured 37 pt text rows and 38.25 pt picker rows. Preserve each window and dialog's source dimensions and adaptive bounds.

Shared controls and their variants live in ui/components.rs; the original SVG family lives in ui/icons.rs. The shared foundations retain the established spacing and radius roles. Screen-specific physical values must be traceable to the native source or recorded AX/pixel measurements, rather than used as arbitrary visual offsets. Colors come from semantic light/dark theme tokens, including control states; keep emphasis, danger and feedback in their original places.

Use NSH semantic controls with accessible names, visible focus and disabled states. Preserve focus, keyboard navigation and dismissal semantics in menus, selectors, dialogs and auxiliary windows. Frequent commands retain Command/Ctrl equivalents. Technical inputs disable autocorrection where native code does. Acceptance actions never scan, connect to, program or control physical devices.

## Motion inventory and implementation status

The migration preserves existing interaction structure. This table records the current application code; it is not a claim that all motion has passed runtime acceptance. Do not add motion or change layout merely to exercise an NSH feature.

| Surface/change | Current implementation | Reduced motion / acceptance |
| --- | --- | --- |
| Shared action buttons | 120 ms color transitions; the 180 ms press transform to 0.97 is guarded by reduce_motion() | Check both settings in the running app; the guard does not certify every other control |
| Icon controls, checkboxes and switches | Their owning shared/local variants define hover, pressed, selected and disabled states; configured switch transitions use the shared durations | Verify only the transitions actually present in each variant |
| Firmware/serial mode and tool tabs | NSH Tabs indicator with a configured 240 ms transform transition | Verify native reduced-motion behavior; no universal pass is assumed |
| Menus and selectors | NSH popup placement, focus and dismissal lifecycle with app-owned popup styling | No application-owned popup scale/slide or starting/ending motion is implemented |
| Secondary dialogs | Centered, bounded NSH modal content and a scrim | No custom arrival/exit animation is implemented; verify framework/system behavior and focus restoration |
| Auxiliary windows | Native NSH window lifecycle | No additional application animation |
| Firmware analysis, results and status | Values and progress update in place | No added movement of technical data |
| Terminal, logs and history | Bounded data with virtualized tables/lists or the established selectable text surface | No row entrance animation on polling, recycling or history updates |
| Plot sampling, pause and clear | NSH plot driven by captured-session sample data | No measurement-value interpolation is configured |
| Session add/remove and connection/composer visibility | Retained state and NSH resizable panel visibility | No application LayoutAnimation has been added |
| Send, repeat and pause labels | Labels follow actual job/display state | No glyph-swap or label-entry animation is implemented |
| Drag/drop and resize | Framework/native pointer and resizable-panel behavior | No additional release spring or easing is configured |
| Sound | None | No sound is required to understand state |

## Verification

Run new Cargo logic tests/clippy and NSH build checks. Manual desktop checks: light/dark, minimum outer window 900×652 (600 pt content plus the 52 pt title bar), controls and focus, tab selection, dialogs (cancel/reopen), independent windows, input drafts and state preservation. Every page, tool tab, dialog and auxiliary window requires paired same-size, same-state screenshots in light and dark, with unscaled pixel differences and recorded source artifact/clone identity. Review radius, typography, spacing/alignment, colors, hover/press, states, motion and keyboard separately; screenshot coverage is not a pixel-parity pass, and unverified groups remain unverified. No GUI automation tests.

## Native panel appearance

Keep each macOS reference entry point's appearance ownership. Standalone AppKit open/save panels and the independent native Checking, Available and Current update windows follow `NSApp`'s system appearance. The main interface follows the application's appearance preference. SwiftUI document import/export sheets follow that preference; the narrow AppKit document bridge applies it to that panel only.

An application Light preference with an OS Dark appearance therefore still produces Dark standalone file and update windows in the current native reference. Do not change OS appearance or inject a Light `NSAppearance` into the reference to manufacture a Light comparison. Debug renderer fixtures remain renderer evidence and do not establish native Available-window or OS-Light acceptance.
