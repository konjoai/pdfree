import SwiftUI
import UniformTypeIdentifiers

/// Minimal iOS shell proving the Rust engine links and runs on-device via
/// the same `pdfree-ffi` UniFFI interface the macOS app uses (see
/// `apps/ios/Frameworks/PdfreeFFI.xcframework`, built by
/// `scripts/build-ios.sh`) — open a PDF, render page 1 at a fit-to-page
/// DPI, display it. This is deliberately **not** a port of the macOS app's
/// views: `apps/macos/Sources/PDFree/Views/*.swift` are written against
/// AppKit (`NSImage`, `NSFont`, `NSPasteboard`, `NSOpenPanel`,
/// `NSFullUserName()`, ...), none of which exist on iOS/UIKit, so literally
/// sharing those files isn't possible without a real cross-platform
/// abstraction layer — out of scope for this pass. What *is* shared is the
/// entire Rust engine and the FFI interface itself: the UniFFI-generated
/// `pdfree_ffi.swift` here is produced from the exact same
/// `crates/pdfree-ffi` crate, unmodified.
///
/// **Known gap, not yet solved**: `pdfree_core::pdfium::bind()`'s native
/// path (vendor-dir search, then system library search) assumes a
/// filesystem/dylib-search environment that doesn't exist inside an iOS
/// app sandbox — there is no `vendor/pdfium/` to find and no system-wide
/// PDFium install to dlopen. A real iOS PDFium integration needs its own
/// bundled `.xcframework` (mirroring `docs/pdfium-bundling.md`'s per-
/// platform strategy) and a `pdfium.rs` binding branch for iOS, the same
/// shape of gap as `apps/web`'s "PDFium WASM module not found" state. This
/// view surfaces that error honestly (via the real FFI error message)
/// rather than hiding it.
struct ContentView: View {
    @State private var pageImage: UIImage?
    @State private var errorMessage: String?
    @State private var showImporter = false

    var body: some View {
        NavigationStack {
            ZStack {
                Color(red: 0.125, green: 0.114, blue: 0.102).ignoresSafeArea()

                if let pageImage {
                    Image(uiImage: pageImage)
                        .resizable()
                        .scaledToFit()
                        .padding()
                } else if let errorMessage {
                    VStack(spacing: 8) {
                        Text("Couldn't open document").font(.headline).foregroundStyle(.white)
                        Text(errorMessage)
                            .font(.footnote)
                            .foregroundStyle(.white.opacity(0.6))
                            .multilineTextAlignment(.center)
                            .padding(.horizontal, 32)
                    }
                } else {
                    VStack(spacing: 14) {
                        Text("PDFree").font(.title2.bold()).foregroundStyle(.white)
                        Button("Open a PDF") { showImporter = true }
                            .buttonStyle(.borderedProminent)
                            .tint(Color(red: 0.216, green: 0.753, blue: 0.478))
                    }
                }
            }
            .fileImporter(isPresented: $showImporter, allowedContentTypes: [.pdf]) { result in
                switch result {
                case .success(let url):
                    openDocument(at: url)
                case .failure(let error):
                    errorMessage = "\(error)"
                }
            }
        }
    }

    private func openDocument(at url: URL) {
        errorMessage = nil
        pageImage = nil

        guard url.startAccessingSecurityScopedResource() else {
            errorMessage = "Couldn't access the selected file."
            return
        }
        defer { url.stopAccessingSecurityScopedResource() }

        do {
            let data = try Data(contentsOf: url)
            let document = try PdfDocument.fromBytes(data: data)
            let size = try document.pageSize(index: 0)
            let dpi = fitToPageDpi(
                pageWidthPts: size.width, pageHeightPts: size.height,
                viewportWidthPx: Float(UIScreen.main.bounds.width * UIScreen.main.scale),
                viewportHeightPx: Float(UIScreen.main.bounds.height * UIScreen.main.scale)
            )
            let png = try document.renderPage(index: 0, dpi: UInt32(dpi))
            pageImage = UIImage(data: png)
        } catch {
            errorMessage = "\(error)"
        }
    }
}

#Preview {
    ContentView()
}
