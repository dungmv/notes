import NotesCore
import SwiftUI

@main
struct NotesApp: App {
    private let version = coreVersion()

    var body: some Scene {
        WindowGroup("Notes") {
            EditorPrototype(version: version)
                .frame(minWidth: 600, minHeight: 420)
        }
        .commands {
            CommandGroup(replacing: .undoRedo) { }
        }
    }
}
