// swift-tools-version: 5.10
import PackageDescription

let package = Package(
    name: "NotesCore",
    platforms: [.macOS(.v14)],
    products: [.library(name: "NotesCore", targets: ["NotesCore"])],
    targets: [
        .binaryTarget(name: "NotesCoreFFI", path: "NotesCoreFFI.xcframework"),
        .target(name: "NotesCore", dependencies: ["NotesCoreFFI"]),
    ]
)
