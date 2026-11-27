import NotesCore
import SwiftUI

@main
struct NotesApp: App {
    private let version = coreVersion()

    var body: some Scene {
        WindowGroup("Notes") {
            ContentUnavailableView {
                Label("Notes", systemImage: "note.text")
            } description: {
                Text("Rust core \(version)")
            }
            .frame(minWidth: 480, minHeight: 320)
        }
    }
}
