import Foundation

/// A signature or set of initials the user has drawn/typed/uploaded once and
/// chosen to save, so a later signature field can be filled in one tap
/// instead of redrawing (see `PDFDocumentStore`'s persistence — PNG blobs +
/// a small JSON index in Application Support).
struct SavedSignature: Identifiable, Equatable {
    enum Kind: String, Equatable {
        case signature
        case initials
    }

    let id: UUID
    let pngData: Data
    let kind: Kind
    let createdAt: Date
}
