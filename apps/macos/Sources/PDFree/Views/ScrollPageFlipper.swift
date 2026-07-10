import AppKit
import SwiftUI

/// Turns trackpad/wheel scrolling (and horizontal swipes) over the canvas into
/// page turns: scroll **down** or **left** → next page; scroll **up** or
/// **right** → previous. A whole-page render fits the viewport, so there's
/// nothing to scroll *within* a page — the gesture is free to mean "next page."
///
/// Implemented with a local scroll-wheel monitor scoped to this view's on-screen
/// frame (via a pass-through NSView used only for geometry), so it never blocks
/// clicks/drags on the page and never fires over the sidebar or inspector. A
/// short cooldown + delta threshold makes one flick advance exactly one page
/// instead of racing through several.
struct ScrollPageFlipper: NSViewRepresentable {
    var onNext: () -> Void
    var onPrev: () -> Void

    func makeCoordinator() -> Coordinator { Coordinator(onNext: onNext, onPrev: onPrev) }

    func makeNSView(context: Context) -> NSView {
        let view = PassthroughView()
        context.coordinator.attach(to: view)
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {
        context.coordinator.onNext = onNext
        context.coordinator.onPrev = onPrev
    }

    static func dismantleNSView(_ nsView: NSView, coordinator: Coordinator) {
        coordinator.detach()
    }

    /// Never participates in hit-testing, so it can fill the canvas for
    /// geometry without swallowing any mouse events meant for the page.
    private final class PassthroughView: NSView {
        override func hitTest(_ point: NSPoint) -> NSView? { nil }
    }

    final class Coordinator {
        var onNext: () -> Void
        var onPrev: () -> Void

        private weak var view: NSView?
        private var monitor: Any?
        private var accumulated: CGFloat = 0
        private var lastFlip = Date.distantPast

        private let threshold: CGFloat = 22
        private let cooldown: TimeInterval = 0.35

        init(onNext: @escaping () -> Void, onPrev: @escaping () -> Void) {
            self.onNext = onNext
            self.onPrev = onPrev
        }

        func attach(to view: NSView) {
            self.view = view
            monitor = NSEvent.addLocalMonitorForEvents(matching: .scrollWheel) { [weak self] event in
                self?.handle(event)
                return event
            }
        }

        func detach() {
            if let monitor { NSEvent.removeMonitor(monitor) }
            monitor = nil
        }

        private func handle(_ event: NSEvent) {
            guard let view, let window = view.window, event.window === window else { return }
            let frameInWindow = view.convert(view.bounds, to: nil)
            guard frameInWindow.contains(event.locationInWindow) else { return }

            let dy = event.hasPreciseScrollingDeltas ? event.scrollingDeltaY : event.deltaY
            let dx = event.hasPreciseScrollingDeltas ? event.scrollingDeltaX : event.deltaX
            // Follow whichever axis dominates this gesture.
            let delta = abs(dy) >= abs(dx) ? dy : dx

            // A new gesture resets the accumulator so momentum from the last
            // one can't leak into this one.
            if event.phase == .began { accumulated = 0 }
            accumulated += delta

            guard Date().timeIntervalSince(lastFlip) > cooldown else { return }
            // Down/left scroll deltas are negative → next; up/right → previous.
            if accumulated <= -threshold {
                accumulated = 0
                lastFlip = Date()
                onNext()
            } else if accumulated >= threshold {
                accumulated = 0
                lastFlip = Date()
                onPrev()
            }
        }
    }
}
