import AppKit
import SwiftUI
import MoonglowKit

/// Records one global hot key: click, press the combination, done.
///
/// Capture runs through a local event monitor, which only sees keys while the app is
/// active — exactly the Settings window's situation. Escape cancels, Delete clears,
/// and a bare key without modifiers is ignored rather than recorded.
struct HotKeyRecorder: View {

    let title: String
    @Binding var shortcut: HotKeyShortcut?

    @State private var recording = false
    @State private var monitor: Any?

    var body: some View {
        HStack(spacing: 6) {
            Button(buttonTitle) { startRecording() }
                .controlSize(.small)
                .disabled(recording)
                .accessibilityLabel("\(title) shortcut")

            if !recording, shortcut != nil {
                Button("Clear") { shortcut = nil }
                    .controlSize(.small)
            }
        }
        .onDisappear { stopRecording() }
    }

    private var buttonTitle: String {
        if recording { return "Press keys…" }
        return shortcut?.displayText ?? "Record"
    }

    private func startRecording() {
        recording = true
        monitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            handle(event: event)
            return event
        }
    }

    private func stopRecording() {
        recording = false
        if let monitor {
            NSEvent.removeMonitor(monitor)
            self.monitor = nil
        }
    }

    private func handle(event: NSEvent) {
        guard recording else { return }

        let modifiers = carbonModifiers(from: event.modifierFlags)
        if event.keyCode == 53, modifiers == 0 {
            stopRecording()
            return
        }
        if event.keyCode == 51 {
            shortcut = nil
            stopRecording()
            return
        }
        guard modifiers != 0 else { return }

        shortcut = HotKeyShortcut(keyCode: UInt32(event.keyCode), modifiers: modifiers)
        stopRecording()
    }

    private func carbonModifiers(from flags: NSEvent.ModifierFlags) -> UInt32 {
        var result: UInt32 = 0
        if flags.contains(.command) { result |= HotKeyShortcut.commandModifier }
        if flags.contains(.option) { result |= HotKeyShortcut.optionModifier }
        if flags.contains(.control) { result |= HotKeyShortcut.controlModifier }
        if flags.contains(.shift) { result |= HotKeyShortcut.shiftModifier }
        return result
    }
}
